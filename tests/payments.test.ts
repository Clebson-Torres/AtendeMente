import { describe, it, expect, vi, beforeEach } from "vitest";
import { api } from "../src/lib/api";

vi.mock("../src/lib/auth", () => ({
  getCurrentToken: () => "test-token",
}));

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("payments list with month/year filter", () => {
  it("includes month and year params when provided", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ success: true, data: [] }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await api.payments.list(3, 2026);

    const url = mockFetch.mock.calls[0][0];
    expect(url).toContain("month=3");
    expect(url).toContain("year=2026");
  });

  it("omits month/year when not provided", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ success: true, data: [] }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await api.payments.list();

    const url = mockFetch.mock.calls[0][0];
    expect(url).not.toContain("month=");
    expect(url).not.toContain("year=");
  });
});

describe("payments summary with month/year filter", () => {
  it("includes month and year params", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ success: true, data: { paid_cents: 0, pending_cents: 0 } }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await api.payments.summary(6, 2026);

    const url = mockFetch.mock.calls[0][0];
    expect(url).toContain("month=6");
    expect(url).toContain("year=2026");
  });

  it("returns correct financial summary shape", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ success: true, data: { paid_cents: 50000, pending_cents: 15000 } }),
    });
    vi.stubGlobal("fetch", mockFetch);

    const result = await api.payments.summary(3, 2026);

    expect(result).toHaveProperty("paid_cents");
    expect(result).toHaveProperty("pending_cents");
    expect(result.paid_cents).toBe(50000);
    expect(result.pending_cents).toBe(15000);
  });
});
