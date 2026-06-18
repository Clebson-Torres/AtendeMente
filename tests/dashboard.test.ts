import { describe, it, expect, vi, beforeEach } from "vitest";
import { api } from "../src/lib/api";

vi.mock("../src/lib/auth", () => ({
  getCurrentToken: () => "test-token",
}));

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("dashboard API", () => {
  it("returns monthly appointments and financial data", async () => {
    const mockData = {
      appointments_count: 10,
      todays_appointments: [],
      upcoming_appointments: [],
      monthly_appointments: [
        { month: "2026-01", count: 5 },
        { month: "2026-02", count: 8 },
        { month: "2026-03", count: 10 },
      ],
      monthly_financial: [
        { month: "2026-01", total_cents: 50000 },
        { month: "2026-02", total_cents: 80000 },
        { month: "2026-03", total_cents: 100000 },
      ],
    };
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ success: true, data: mockData }),
    });
    vi.stubGlobal("fetch", mockFetch);

    const result = await api.dashboard();

    expect(result).toHaveProperty("monthly_appointments");
    expect(result).toHaveProperty("monthly_financial");
    expect(result.monthly_appointments).toHaveLength(3);
    expect(result.monthly_financial).toHaveLength(3);
    expect(result.monthly_appointments[0].month).toBe("2026-01");
    expect(result.monthly_appointments[0].count).toBe(5);
    expect(result.monthly_financial[0].total_cents).toBe(50000);
  });

  it("returns empty arrays when no data", async () => {
    const mockData = {
      appointments_count: 0,
      todays_appointments: [],
      upcoming_appointments: [],
      monthly_appointments: [],
      monthly_financial: [],
    };
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ success: true, data: mockData }),
    });
    vi.stubGlobal("fetch", mockFetch);

    const result = await api.dashboard();

    expect(result.monthly_appointments).toEqual([]);
    expect(result.monthly_financial).toEqual([]);
  });
});
