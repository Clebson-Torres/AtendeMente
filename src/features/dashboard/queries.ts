import "server-only";
import { and, eq, gte, isNull, lt, sql } from "drizzle-orm";
import { getDb } from "@/db/client";
import { appointments } from "@/db/schema";
import { listTodaysAppointments, listUpcomingAppointments } from "@/features/appointments/queries";

export async function getDashboardData(userId: string) {
  const db = getDb();
  const monthStart = new Date();
  monthStart.setDate(1);
  monthStart.setHours(0, 0, 0, 0);

  const monthEnd = new Date(monthStart);
  monthEnd.setMonth(monthStart.getMonth() + 1);

  const [stats] = await db
    .select({
      appointmentsCount:
        sql<number>`count(*) filter (where ${appointments.status} in ('scheduled', 'completed'))`.mapWith(Number),
    })
    .from(appointments)
    .where(
      and(
        eq(appointments.userId, userId),
        isNull(appointments.deletedAt),
        gte(appointments.startsAt, monthStart),
        lt(appointments.startsAt, monthEnd)
      )
    );

  return {
    upcomingAppointments: await listUpcomingAppointments(userId),
    todaysAppointments: await listTodaysAppointments(userId),
    financialSummary: {
      appointmentsCount: stats?.appointmentsCount ?? 0,
    },
  };
}
