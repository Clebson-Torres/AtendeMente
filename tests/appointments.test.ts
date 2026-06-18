import { describe, it, expect } from "vitest";

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
