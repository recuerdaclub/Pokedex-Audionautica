import { describe, expect, it } from "vitest";
import {
  formatPlayerTime,
  previewDoesNotChangeSelection,
  ratioFromTime,
  seekFromRatio,
  stopResetsTime,
  transitionSource,
} from "./reviewPlayerState";

describe("reviewPlayerState", () => {
  it("formats player time", () => {
    expect(formatPlayerTime(17)).toBe("00:17");
    expect(formatPlayerTime(68)).toBe("01:08");
  });

  it("seek from ratio", () => {
    expect(seekFromRatio(0.75, 68)).toBeCloseTo(51, 5);
    expect(seekFromRatio(0, 10)).toBe(0);
  });

  it("ratio from time", () => {
    expect(ratioFromTime(51, 68)).toBeCloseTo(0.75, 5);
  });

  it("stop resets to zero", () => {
    expect(stopResetsTime()).toBe(0);
  });

  it("transition source autoplay when was playing", () => {
    const t = transitionSource("a", "b", true);
    expect(t.shouldAutoplay).toBe(true);
    expect(t.nextKey).toBe("b");
  });

  it("preview does not mutate selection", () => {
    const before = { selected: true, category: "OTHER" as const };
    const after = { ...before };
    expect(previewDoesNotChangeSelection(before, after)).toBe(true);
  });
});
