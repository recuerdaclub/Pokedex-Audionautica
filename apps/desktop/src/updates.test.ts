import { describe, expect, it } from "vitest";
import {
  buildUpdateInfo,
  checkForUpdates,
  compareSemver,
  fetchLatestRelease,
  isNewerVersion,
  parseVersionFromTag,
  pickInstallerAsset,
  type GitHubRelease,
} from "./updates";

const sampleRelease: GitHubRelease = {
  tag_name: "v1.0.6",
  name: "Audionautica 1.0.6",
  html_url: "https://github.com/recuerdaclub/Pokedex-Audionautica/releases/tag/v1.0.6",
  body: "Notas de prueba",
  assets: [
    {
      name: "Audionautica_1.0.4_universal.dmg",
      browser_download_url: "https://example.com/old.dmg",
      size: 100,
    },
    {
      name: "Audionautica_1.0.6_x64-setup.exe",
      browser_download_url: "https://example.com/setup.exe",
      size: 200,
    },
    {
      name: "Audionautica_1.0.6_x64_en-US.msi",
      browser_download_url: "https://example.com/app.msi",
      size: 300,
    },
    {
      name: "Audionautica_1.0.6_universal.dmg",
      browser_download_url: "https://example.com/universal.dmg",
      size: 400,
    },
    {
      name: "SHA256SUMS.txt",
      browser_download_url: "https://example.com/sha.txt",
      size: 10,
    },
  ],
};

describe("updates", () => {
  it("parses tag versions", () => {
    expect(parseVersionFromTag("v1.0.5")).toBe("1.0.5");
    expect(parseVersionFromTag("1.0.5")).toBe("1.0.5");
  });

  it("compares semver", () => {
    expect(compareSemver("1.0.6", "1.0.5")).toBe(1);
    expect(compareSemver("1.0.5", "1.0.5")).toBe(0);
    expect(compareSemver("1.0.4", "1.0.5")).toBe(-1);
    expect(compareSemver("1.1.0", "1.0.9")).toBe(1);
  });

  it("detects newer versions", () => {
    expect(isNewerVersion("1.0.6", "1.0.5")).toBe(true);
    expect(isNewerVersion("1.0.5", "1.0.5")).toBe(false);
  });

  it("picks windows installer for matching release version", () => {
    const asset = pickInstallerAsset(sampleRelease.assets, "windows", "1.0.6");
    expect(asset?.name).toBe("Audionautica_1.0.6_x64_en-US.msi");
  });

  it("picks macos universal dmg for matching release version", () => {
    const asset = pickInstallerAsset(sampleRelease.assets, "macos", "1.0.6");
    expect(asset?.name).toBe("Audionautica_1.0.6_universal.dmg");
  });

  it("builds update info when a newer release exists", () => {
    const info = buildUpdateInfo(sampleRelease, "1.0.5", "windows");
    expect(info.latestVersion).toBe("1.0.6");
    expect(info.asset?.downloadUrl).toContain("app.msi");
    expect(isNewerVersion(info.latestVersion, info.currentVersion)).toBe(true);
  });
});

describe("updates github integration", () => {
  it("connects to GitHub releases API", async () => {
    const release = await fetchLatestRelease();
    expect(release.tag_name).toMatch(/^v?\d+\.\d+\.\d+$/i);
    expect(release.assets.length).toBeGreaterThan(0);
  }, 15_000);

  it("reports up-to-date for the current published release", async () => {
    const release = await fetchLatestRelease();
    const latest = parseVersionFromTag(release.tag_name);
    const result = await checkForUpdates(latest);
    expect(result.status).toBe("up-to-date");
  }, 15_000);
});
