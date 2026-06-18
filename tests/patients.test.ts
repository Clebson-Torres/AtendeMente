import { describe, it, expect, vi, beforeEach } from "vitest";
import { api } from "../src/lib/api";

vi.mock("../src/lib/auth", () => ({
  getCurrentToken: () => "test-token",
}));

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("patients list with status filter", () => {
  it("includes status param when provided", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ success: true, data: { items: [], total: 0, page: 1, per_page: 50 } }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await api.patients.list("", 1, 50, "active");

    expect(mockFetch).toHaveBeenCalled();
    const url = mockFetch.mock.calls[0][0];
    expect(url).toContain("status=active");
  });

  it("omits status param when not provided", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ success: true, data: { items: [], total: 0, page: 1, per_page: 50 } }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await api.patients.list();

    const url = mockFetch.mock.calls[0][0];
    expect(url).not.toContain("status=");
  });
});
