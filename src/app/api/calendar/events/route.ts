import { NextRequest, NextResponse } from "next/server";
import { listCalendarEvents } from "@/features/appointments/queries";
import { getCurrentUser } from "@/lib/auth/session";

function getEventColors(confirmationStatus: "unconfirmed" | "confirmed" | "cancelled") {
  if (confirmationStatus === "confirmed") {
    return {
      backgroundColor: "#15803d",
      borderColor: "#166534",
    };
  }

  if (confirmationStatus === "cancelled") {
    return {
      backgroundColor: "#dc2626",
      borderColor: "#b91c1c",
    };
  }

  return {
    backgroundColor: "#d97706",
    borderColor: "#b45309",
  };
}

export async function GET(request: NextRequest) {
  const user = await getCurrentUser();

  if (!user) {
    return NextResponse.json({ message: "Nao autenticado." }, { status: 401 });
  }

  const start = request.nextUrl.searchParams.get("start");
  const end = request.nextUrl.searchParams.get("end");

  if (!start || !end) {
    return NextResponse.json({ message: "Periodo invalido." }, { status: 400 });
  }

  const events = await listCalendarEvents(user.id, new Date(start), new Date(end));

  return NextResponse.json({
    events: events.map((event) => ({
      id: event.id,
      patientId: event.patientId,
      title: event.title,
      start: event.start,
      end: event.end,
      status: event.status,
      confirmationStatus: event.confirmationStatus,
      ...getEventColors(event.confirmationStatus),
    })),
  });
}
