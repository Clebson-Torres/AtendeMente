import { date, index, pgEnum, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";
import { users } from "@/db/schema/users";

export const patientStatusEnum = pgEnum("patient_status", ["active", "inactive"]);

export const patients = pgTable(
  "patients",
  {
    id: uuid("id").defaultRandom().primaryKey(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id, { onDelete: "cascade" }),
    fullName: text("full_name").notNull(),
    chartNumber: text("chart_number"),
    phone: text("phone"),
    email: text("email"),
    birthDate: date("birth_date"),
    status: patientStatusEnum("status").notNull().default("active"),
    healthHistory: text("health_history"),
    medicationsInUse: text("medications_in_use"),
    emergencyPhone: text("emergency_phone"),
    adminNotes: text("admin_notes"),
    createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true }).notNull().defaultNow(),
    deletedAt: timestamp("deleted_at", { withTimezone: true }),
  },
  (table) => ({
    userIdx: index("patients_user_idx").on(table.userId),
    chartNumberIdx: index("patients_chart_number_idx").on(table.chartNumber),
    statusIdx: index("patients_status_idx").on(table.status),
    deletedIdx: index("patients_deleted_idx").on(table.deletedAt),
  }),
);
