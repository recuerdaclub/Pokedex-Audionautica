export const GITHUB_REPO = "recuerdaclub/Pokedex-Audionautica";

export const GITHUB_RELEASES_LATEST_URL =
  `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`;

export type AppPlatform = "windows" | "macos" | "linux" | "unknown";

export type UpdateCheckStatus = "idle" | "checking" | "up-to-date" | "available" | "error";

export interface GitHubReleaseAsset {
  name: string;
  browser_download_url: string;
  size: number;
}

export interface GitHubRelease {
  tag_name: string;
  name: string;
  html_url: string;
  body: string;
  assets: GitHubReleaseAsset[];
}

export interface UpdateAsset {
  name: string;
  downloadUrl: string;
  size: number;
}

export interface UpdateInfo {
  currentVersion: string;
  latestVersion: string;
  releaseName: string;
  releaseNotes: string;
  releaseUrl: string;
  asset: UpdateAsset | null;
  platform: AppPlatform;
}

export interface UpdateCheckResult {
  status: "up-to-date" | "available";
  info: UpdateInfo;
}

export function detectPlatform(): AppPlatform {
  if (typeof navigator === "undefined") return "unknown";
  const ua = navigator.userAgent;
  if (/Win/i.test(ua)) return "windows";
  if (/Mac/i.test(ua)) return "macos";
  if (/Linux/i.test(ua)) return "linux";
  return "unknown";
}

export function parseVersionFromTag(tag: string): string {
  return tag.trim().replace(/^v/i, "");
}

export function compareSemver(a: string, b: string): -1 | 0 | 1 {
  const pa = a.split(".").map((part) => Number.parseInt(part, 10) || 0);
  const pb = b.split(".").map((part) => Number.parseInt(part, 10) || 0);
  const len = Math.max(pa.length, pb.length);

  for (let i = 0; i < len; i += 1) {
    const na = pa[i] ?? 0;
    const nb = pb[i] ?? 0;
    if (na > nb) return 1;
    if (na < nb) return -1;
  }

  return 0;
}

export function isNewerVersion(latest: string, current: string): boolean {
  return compareSemver(latest, current) > 0;
}

function assetMatchesVersion(name: string, version: string): boolean {
  const normalized = name.toLowerCase();
  const ver = version.toLowerCase();
  return normalized.includes(ver) || normalized.includes(ver.replace(/\./g, "_"));
}

function isInstallerAsset(name: string): boolean {
  const lower = name.toLowerCase();
  return (
    lower.endsWith(".msi") ||
    lower.endsWith("-setup.exe") ||
    lower.endsWith(".dmg") ||
    lower.endsWith(".exe")
  );
}

function toUpdateAsset(asset: GitHubReleaseAsset): UpdateAsset {
  return {
    name: asset.name,
    downloadUrl: asset.browser_download_url,
    size: asset.size,
  };
}

function pickFirstMatching(
  assets: GitHubReleaseAsset[],
  predicate: (asset: GitHubReleaseAsset) => boolean,
): UpdateAsset | null {
  const match = assets.find(predicate);
  return match ? toUpdateAsset(match) : null;
}

export function pickInstallerAsset(
  assets: GitHubReleaseAsset[],
  platform: AppPlatform,
  releaseVersion: string,
): UpdateAsset | null {
  const installers = assets.filter((asset) => isInstallerAsset(asset.name));
  const forVersion = installers.filter((asset) => assetMatchesVersion(asset.name, releaseVersion));

  if (platform === "windows") {
    return (
      pickFirstMatching(forVersion, (a) => /\.msi$/i.test(a.name) && /x64/i.test(a.name)) ??
      pickFirstMatching(forVersion, (a) => /-setup\.exe$/i.test(a.name)) ??
      pickFirstMatching(forVersion, (a) => /\.msi$/i.test(a.name)) ??
      pickFirstMatching(forVersion, (a) => /-setup\.exe$/i.test(a.name)) ??
      pickFirstMatching(installers, (a) => /\.msi$/i.test(a.name)) ??
      pickFirstMatching(installers, (a) => /-setup\.exe$/i.test(a.name))
    );
  }

  if (platform === "macos") {
    return (
      pickFirstMatching(forVersion, (a) => /universal\.dmg$/i.test(a.name)) ??
      pickFirstMatching(forVersion, (a) => /arm64\.dmg$/i.test(a.name)) ??
      pickFirstMatching(forVersion, (a) => /x86_64\.dmg$/i.test(a.name)) ??
      pickFirstMatching(forVersion, (a) => /\.dmg$/i.test(a.name)) ??
      pickFirstMatching(installers, (a) => /universal\.dmg$/i.test(a.name)) ??
      pickFirstMatching(installers, (a) => /\.dmg$/i.test(a.name))
    );
  }

  return null;
}

export function buildUpdateInfo(
  release: GitHubRelease,
  currentVersion: string,
  platform: AppPlatform = detectPlatform(),
): UpdateInfo {
  const latestVersion = parseVersionFromTag(release.tag_name);
  return {
    currentVersion,
    latestVersion,
    releaseName: release.name,
    releaseNotes: release.body?.trim() ?? "",
    releaseUrl: release.html_url,
    asset: pickInstallerAsset(release.assets, platform, latestVersion),
    platform,
  };
}

export async function fetchLatestRelease(
  fetchImpl: typeof fetch = fetch,
): Promise<GitHubRelease> {
  const response = await fetchImpl(GITHUB_RELEASES_LATEST_URL, {
    headers: {
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
      "User-Agent": "Pokedex-Audionautica-Updater",
    },
  });

  if (!response.ok) {
    throw new Error(
      response.status === 404
        ? "No hay releases publicados en GitHub todavía."
        : `No se pudo consultar GitHub (${response.status}).`,
    );
  }

  return (await response.json()) as GitHubRelease;
}

export async function checkForUpdates(
  currentVersion: string,
  fetchImpl: typeof fetch = fetch,
  platform: AppPlatform = detectPlatform(),
): Promise<UpdateCheckResult> {
  const release = await fetchLatestRelease(fetchImpl);
  const info = buildUpdateInfo(release, currentVersion, platform);

  if (!isNewerVersion(info.latestVersion, currentVersion)) {
    return { status: "up-to-date", info };
  }

  return { status: "available", info };
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export async function getInstalledVersion(): Promise<string> {
  try {
    const { getVersion } = await import("@tauri-apps/api/app");
    return await getVersion();
  } catch {
    return typeof __APP_VERSION__ !== "undefined" ? __APP_VERSION__ : "0.0.0";
  }
}

export async function openExternalUrl(url: string): Promise<void> {
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

export function platformInstallInstructions(platform: AppPlatform): string[] {
  if (platform === "windows") {
    return [
      "Se descargará el instalador (.msi o .exe). Ejecútalo cuando termine.",
      "Si Windows SmartScreen advierte, elige Más información → Ejecutar de todas formas.",
      "Confirma el permiso de administrador (UAC) para completar la instalación.",
    ];
  }

  if (platform === "macos") {
    return [
      "Se descargará el archivo .dmg. Ábrelo y arrastra Pokedex Audionautica a Aplicaciones.",
      "Si macOS bloquea la app: Ajustes del Sistema → Privacidad y seguridad → Abrir igual.",
      "Si la instalación automática falla, usa Ver en GitHub e instala manualmente.",
    ];
  }

  return [
    "Descarga el instalador desde GitHub y sigue las instrucciones de la release.",
  ];
}
