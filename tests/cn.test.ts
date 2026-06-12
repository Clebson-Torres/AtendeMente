import { describe, it, expect } from "vitest";
import { cn } from "../src/lib/utils";

describe("cn", () => {
  it("merges class names", () => {
    expect(cn("px-4", "py-2")).toBe("px-4 py-2");
  });

  it("handles conditional classes", () => {
    expect(cn("base", false && "hidden", "visible")).toBe("base visible");
  });

  it("merges tailwind conflicts correctly", () => {
    expect(cn("px-4", "px-2")).toBe("px-2");
    expect(cn("text-red-500", "text-blue-600")).toBe("text-blue-600");
  });

  it("handles empty args", () => {
    expect(cn()).toBe("");
  });
});
