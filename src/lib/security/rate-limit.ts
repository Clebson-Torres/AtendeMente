import "server-only";
import { and, eq } from "drizzle-orm";
import { getDb } from "@/db/client";
import { requestLimits } from "@/db/schema";
import { AppError } from "@/lib/errors/app-error";

type RateLimitInput = {
  scope: string;
  identifier: string;
  limit: number;
  windowMs: number;
};

function isMissingRateLimitTableError(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    error.code === "42P01"
  );
}

function throwRateLimitUnavailable(error: unknown): never {
  console.error("Rate limit infrastructure unavailable", error);
  throw new AppError("Servico temporariamente indisponivel.", {
    statusCode: 503,
    code: "RATE_LIMIT_UNAVAILABLE",
  });
}

export async function enforceRateLimit({ scope, identifier, limit, windowMs }: RateLimitInput) {
  const db = getDb();
  const now = new Date();

  let existing:
    | {
        id: string;
        hits: number;
        windowStartsAt: Date;
      }
    | undefined;

  try {
    existing = await db.query.requestLimits.findFirst({
      where: and(eq(requestLimits.scope, scope), eq(requestLimits.identifier, identifier)),
      columns: {
        id: true,
        hits: true,
        windowStartsAt: true,
      },
    });
  } catch (error) {
    if (isMissingRateLimitTableError(error)) {
      throwRateLimitUnavailable(error);
    }

    throw error;
  }

  if (!existing) {
    try {
      await db.insert(requestLimits).values({
        scope,
        identifier,
        hits: 1,
        windowStartsAt: now,
      });
    } catch (error) {
      if (isMissingRateLimitTableError(error)) {
        throwRateLimitUnavailable(error);
      }

      throw error;
    }

    return;
  }

  const elapsed = now.getTime() - new Date(existing.windowStartsAt).getTime();

  if (elapsed >= windowMs) {
    await db
      .update(requestLimits)
      .set({
        hits: 1,
        windowStartsAt: now,
        updatedAt: now,
      })
      .where(eq(requestLimits.id, existing.id));
    return;
  }

  if (existing.hits >= limit) {
    throw new AppError("Muitas tentativas em pouco tempo. Tente novamente em alguns minutos.", {
      statusCode: 429,
      code: "RATE_LIMITED",
    });
  }

  try {
    await db
      .update(requestLimits)
      .set({
        hits: existing.hits + 1,
        updatedAt: now,
      })
      .where(eq(requestLimits.id, existing.id));
  } catch (error) {
    if (isMissingRateLimitTableError(error)) {
      throwRateLimitUnavailable(error);
    }

    throw error;
  }
}
