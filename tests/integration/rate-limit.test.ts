import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppError } from "@/lib/errors/app-error";
import { enforceRateLimit } from "@/lib/security/rate-limit";

const findFirst = vi.fn();
const insertValues = vi.fn();
const updateSet = vi.fn();
const updateWhere = vi.fn();

vi.mock("@/db/client", () => ({
  getDb: () => ({
    query: {
      requestLimits: {
        findFirst,
      },
    },
    insert: () => ({
      values: insertValues,
    }),
    update: () => ({
      set: updateSet,
    }),
  }),
}));

beforeEach(() => {
  findFirst.mockReset();
  insertValues.mockReset();
  updateSet.mockReset();
  updateWhere.mockReset();
  updateSet.mockReturnValue({ where: updateWhere });
});

describe("rate limit", () => {
  it("fails closed when the request_limits table is missing", async () => {
    findFirst.mockRejectedValue({ code: "42P01" });

    await expect(
      enforceRateLimit({
        scope: "auth:login",
        identifier: "qa@atendemente.local",
        limit: 5,
        windowMs: 1_000,
      }),
    ).rejects.toMatchObject<AppError>({
      code: "RATE_LIMIT_INFRA_UNAVAILABLE",
      statusCode: 503,
    });
  });

  it("returns 429 when the window is exhausted", async () => {
    findFirst.mockResolvedValue({
      id: "limit-1",
      hits: 5,
      windowStartsAt: new Date(),
    });

    await expect(
      enforceRateLimit({
        scope: "auth:login",
        identifier: "qa@atendemente.local",
        limit: 5,
        windowMs: 10_000,
      }),
    ).rejects.toMatchObject<AppError>({
      code: "RATE_LIMITED",
      statusCode: 429,
    });
  });
});
