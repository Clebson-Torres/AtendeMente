import { beforeEach, describe, expect, it, vi } from "vitest";

const requireUser = vi.fn();
const getDb = vi.fn();
const writeAuditLog = vi.fn();
const revalidatePath = vi.fn();

vi.mock("@/lib/auth/session", () => ({
  requireUser,
}));

vi.mock("@/db/client", () => ({
  getDb,
}));

vi.mock("@/lib/audit/log", () => ({
  writeAuditLog,
}));

vi.mock("next/cache", () => ({
  revalidatePath,
}));

function createSelectResult(value: unknown) {
  const result = Promise.resolve(value);
  return Object.assign(
    {
      limit: vi.fn(() => Promise.resolve(value)),
      then: result.then.bind(result),
      catch: result.catch.bind(result),
      finally: result.finally.bind(result),
      [Symbol.toStringTag]: "Promise",
    },
  );
}

function createDbMock(options?: {
  selectResults?: unknown[];
  insertResult?: unknown[];
  updateResult?: unknown[];
}) {
  const selectResults = [...(options?.selectResults ?? [])];
  const insertResult = options?.insertResult ?? [{ id: "patient-1" }];
  const updateResult = options?.updateResult ?? [{ id: "patient-1" }];

  const select = vi.fn(() => ({
    from: vi.fn(() => ({
      where: vi.fn(() => createSelectResult(selectResults.shift() ?? [])),
    })),
  }));

  const insertReturning = vi.fn(async () => insertResult);
  const insertValues = vi.fn(() => ({
    returning: insertReturning,
  }));
  const insert = vi.fn(() => ({
    values: insertValues,
  }));

  const updateReturning = vi.fn(async () => updateResult);
  const updateWhere = vi.fn(() => ({
    returning: updateReturning,
  }));
  const updateSet = vi.fn(() => ({
    where: updateWhere,
  }));
  const update = vi.fn(() => ({
    set: updateSet,
  }));

  return {
    db: {
      select,
      insert,
      update,
    },
    insertValues,
    updateSet,
  };
}

describe("patient actions", () => {
  beforeEach(() => {
    vi.resetModules();
    requireUser.mockReset();
    getDb.mockReset();
    writeAuditLog.mockReset();
    revalidatePath.mockReset();
  });

  it("creates a patient with trimmed chart number", async () => {
    const dbMock = createDbMock({ selectResults: [[], []] });
    getDb.mockReturnValue(dbMock.db);
    requireUser.mockResolvedValue({ id: "user-1" });

    const { createPatientAction } = await import("@/features/patients/actions");
    const result = await createPatientAction({
      fullName: "Marina Alves",
      chartNumber: "  PR-001  ",
      phone: "11999998888",
      email: "marina@email.com",
      birthDate: "1992-08-12",
      healthHistory: "",
      medicationsInUse: "",
      emergencyPhone: "",
      adminNotes: "",
    });

    expect(result.success).toBe(true);
    expect(dbMock.insertValues).toHaveBeenCalledWith(
      expect.objectContaining({
        userId: "user-1",
        chartNumber: "PR-001",
      }),
    );
  }, 30_000);

  it("blocks duplicate chart number on create", async () => {
    const dbMock = createDbMock({
      selectResults: [
        [],
        [{ id: "patient-2", chartNumber: "PR-001" }],
      ],
    });
    getDb.mockReturnValue(dbMock.db);
    requireUser.mockResolvedValue({ id: "user-1" });

    const { createPatientAction } = await import("@/features/patients/actions");
    const result = await createPatientAction({
      fullName: "Marina Alves",
      chartNumber: "PR-001",
      phone: "11999998888",
      email: "marina@email.com",
      birthDate: "1992-08-12",
      healthHistory: "",
      medicationsInUse: "",
      emergencyPhone: "",
      adminNotes: "",
    });

    expect(result.success).toBe(false);
    expect(result.message).toContain("numero do prontuario");
  }, 30_000);

  it("updates the patient chart number", async () => {
    const dbMock = createDbMock({ selectResults: [[], []] });
    getDb.mockReturnValue(dbMock.db);
    requireUser.mockResolvedValue({ id: "user-1" });

    const { updatePatientAction } = await import("@/features/patients/actions");
    const result = await updatePatientAction("patient-1", {
      fullName: "Marina Alves",
      chartNumber: "PR-9001",
      phone: "11999998888",
      email: "marina@email.com",
      birthDate: "1992-08-12",
      healthHistory: "",
      medicationsInUse: "",
      emergencyPhone: "",
      adminNotes: "",
    });

    expect(result.success).toBe(true);
    expect(dbMock.updateSet).toHaveBeenCalledWith(
      expect.objectContaining({
        chartNumber: "PR-9001",
      }),
    );
  }, 30_000);
});
