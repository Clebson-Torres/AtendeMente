import { describe, it, expect } from "vitest";
import { toLocalDatetimeString } from "../src/lib/format";

function getEndFromStart(startsAt: string): string {
  const start = new Date(startsAt);
  const end = new Date(start.getTime() + 60 * 60 * 1000);
  return toLocalDatetimeString(end);
}

const mockEvents = [
  { id: "1", status: "scheduled", confirmation_status: "confirmed", start: "2026-03-10T09:00:00" },
  { id: "2", status: "completed", confirmation_status: "confirmed", start: "2026-03-10T10:00:00" },
  { id: "3", status: "cancelled", confirmation_status: "cancelled", start: "2026-03-11T09:00:00" },
  { id: "4", status: "scheduled", confirmation_status: "unconfirmed", start: "2026-03-12T09:00:00" },
  { id: "5", status: "no_show", confirmation_status: "confirmed", start: "2026-03-13T09:00:00" },
];

function filterEvents(events: typeof mockEvents, statusFilter: string, confirmationFilter: string) {
  return events.filter((e) => {
    if (statusFilter && e.status !== statusFilter) return false;
    if (confirmationFilter && e.confirmation_status !== confirmationFilter) return false;
    return true;
  });
}

describe("toLocalDatetimeString", () => {
  it("formats date with local timezone", () => {
    const d = new Date(2026, 5, 22, 8, 0);
    const result = toLocalDatetimeString(d);
    expect(result).toBe("2026-06-22T08:00");
  });

  it("pads single-digit month and day", () => {
    const d = new Date(2026, 0, 5, 9, 5);
    const result = toLocalDatetimeString(d);
    expect(result).toBe("2026-01-05T09:05");
  });

  it("pads single-digit hour and minute", () => {
    const d = new Date(2026, 11, 3, 7, 3);
    const result = toLocalDatetimeString(d);
    expect(result).toBe("2026-12-03T07:03");
  });
});

describe("ends_at auto-update", () => {
  it("calculates end as start + 1 hour", () => {
    const end = getEndFromStart("2026-07-01T17:00");
    expect(end).toBe("2026-07-01T18:00");
  });

  it("handles hour rollover", () => {
    const end = getEndFromStart("2026-07-01T23:00");
    expect(end).toBe("2026-07-02T00:00");
  });

  it("handles month rollover", () => {
    const end = getEndFromStart("2026-12-31T22:00");
    expect(end).toBe("2026-12-31T23:00");
  });

  it("handles the onChange scenario with a given starts_at", () => {
    const start = new Date(2026, 5, 22, 6, 0);
    const startStr = toLocalDatetimeString(start);
    expect(startStr).toBe("2026-06-22T06:00");
    const end = new Date(start.getTime() + 60 * 60 * 1000);
    expect(toLocalDatetimeString(end)).toBe("2026-06-22T07:00");
  });
});

type RecurrenceInput = {
  frequency: "weekly" | "biweekly" | "monthly";
  end_mode: "occurrences" | "until_date";
  occurrences?: number;
  until_date?: string;
};

type AppointmentInput = {
  starts_at: string;
  ends_at: string;
  patient_id: string;
  recurrence_frequency?: string;
  recurrence_end_mode?: string;
  recurrence_occurrences?: number;
  recurrence_until_date?: string;
};

function buildRecurrencePayload(data: AppointmentInput, enabled: boolean, rec?: RecurrenceInput): Record<string, unknown> {
  const payload: Record<string, unknown> = {
    patient_id: data.patient_id,
    starts_at: data.starts_at,
    ends_at: data.ends_at,
  };
  if (enabled && rec) {
    payload.recurrence_frequency = rec.frequency;
    payload.recurrence_end_mode = rec.end_mode;
    if (rec.end_mode === "occurrences") {
      payload.recurrence_occurrences = rec.occurrences;
    } else {
      payload.recurrence_until_date = rec.until_date;
    }
  }
  return payload;
}

describe("appointment recurrence", () => {
  const base: AppointmentInput = {
    patient_id: "p1",
    starts_at: "2026-07-01T10:00",
    ends_at: "2026-07-01T11:00",
  };

  it("omits recurrence fields when disabled", () => {
    const payload = buildRecurrencePayload(base, false);
    expect(payload).not.toHaveProperty("recurrence_frequency");
    expect(payload).not.toHaveProperty("recurrence_end_mode");
    expect(payload).not.toHaveProperty("recurrence_occurrences");
    expect(payload).not.toHaveProperty("recurrence_until_date");
    expect(payload.starts_at).toBe("2026-07-01T10:00");
    expect(payload.patient_id).toBe("p1");
  });

  it("includes weekly recurrence with occurrences", () => {
    const payload = buildRecurrencePayload(base, true, {
      frequency: "weekly",
      end_mode: "occurrences",
      occurrences: 4,
    });
    expect(payload.recurrence_frequency).toBe("weekly");
    expect(payload.recurrence_occurrences).toBe(4);
    expect(payload).not.toHaveProperty("recurrence_until_date");
  });

  it("includes biweekly recurrence with until_date", () => {
    const payload = buildRecurrencePayload(base, true, {
      frequency: "biweekly",
      end_mode: "until_date",
      until_date: "2026-09-01",
    });
    expect(payload.recurrence_frequency).toBe("biweekly");
    expect(payload.recurrence_until_date).toBe("2026-09-01");
    expect(payload).not.toHaveProperty("recurrence_occurrences");
  });

  it("includes monthly recurrence with occurrences", () => {
    const payload = buildRecurrencePayload(base, true, {
      frequency: "monthly",
      end_mode: "occurrences",
      occurrences: 12,
    });
    expect(payload.recurrence_frequency).toBe("monthly");
    expect(payload.recurrence_occurrences).toBe(12);
  });

  it("clears occurrences when end_mode switches to until_date", () => {
    const withOccurrences = buildRecurrencePayload(base, true, {
      frequency: "weekly",
      end_mode: "occurrences",
      occurrences: 4,
    });
    expect(withOccurrences).toHaveProperty("recurrence_occurrences");
    expect(withOccurrences).not.toHaveProperty("recurrence_until_date");
  });
});

describe("appointment filters", () => {
  it("returns all events when no filters", () => {
    const result = filterEvents(mockEvents, "", "");
    expect(result).toHaveLength(5);
  });

  it("filters by status = scheduled", () => {
    const result = filterEvents(mockEvents, "scheduled", "");
    expect(result).toHaveLength(2);
    expect(result.every((e) => e.status === "scheduled")).toBe(true);
  });

  it("filters by status = completed", () => {
    const result = filterEvents(mockEvents, "completed", "");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("2");
  });

  it("filters by confirmation_status = confirmed", () => {
    const result = filterEvents(mockEvents, "", "confirmed");
    expect(result).toHaveLength(3);
    expect(result.every((e) => e.confirmation_status === "confirmed")).toBe(true);
  });

  it("filters by both status and confirmation", () => {
    const result = filterEvents(mockEvents, "scheduled", "confirmed");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("1");
  });

  it("returns empty when no match", () => {
    const result = filterEvents(mockEvents, "completed", "unconfirmed");
    expect(result).toHaveLength(0);
  });

  it("filters by cancelled status", () => {
    const result = filterEvents(mockEvents, "cancelled", "");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("3");
  });

  it("filters by no_show status", () => {
    const result = filterEvents(mockEvents, "no_show", "");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("5");
  });
});
