"use client";

import { useEffect, useRef, useState } from "react";
import FullCalendar from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/daygrid";
import interactionPlugin from "@fullcalendar/interaction";
import timeGridPlugin from "@fullcalendar/timegrid";
import ptBrLocale from "@fullcalendar/core/locales/pt-br";
import { useRouter } from "next/navigation";
import type { DateSelectArg, EventClickArg, EventSourceFuncArg } from "@fullcalendar/core";

type CalendarEvent = {
  id: string;
  title: string;
  start: string;
  end: string;
  status: string;
  confirmationStatus: string;
  backgroundColor?: string;
  borderColor?: string;
};

type AppointmentsCalendarProps = {
  onSlotSelect?: (slot: { start: string; end: string }) => void;
};

export function AppointmentsCalendar({ onSlotSelect }: AppointmentsCalendarProps) {
  const router = useRouter();
  const calendarRef = useRef<FullCalendar | null>(null);
  const [isCompact, setIsCompact] = useState(false);

  useEffect(() => {
    const mediaQuery = window.matchMedia("(max-width: 768px)");
    const sync = () => setIsCompact(mediaQuery.matches);

    sync();
    mediaQuery.addEventListener("change", sync);

    return () => mediaQuery.removeEventListener("change", sync);
  }, []);

  const handleEventClick = (eventInfo: EventClickArg) => {
    router.push(`/appointments/${eventInfo.event.id}`);
  };

  const handleSelect = (selectionInfo: DateSelectArg) => {
    onSlotSelect?.({
      start: selectionInfo.startStr,
      end: selectionInfo.endStr,
    });
  };

  const loadEvents = async (
    fetchInfo: EventSourceFuncArg,
    successCallback: (events: CalendarEvent[]) => void,
    failureCallback: (error: Error) => void,
  ) => {
    try {
      const response = await fetch(`/api/calendar/events?start=${fetchInfo.startStr}&end=${fetchInfo.endStr}`);

      if (!response.ok) {
        throw new Error("Nao foi possivel carregar a agenda.");
      }

      const payload = await response.json();
      successCallback(payload.events);
    } catch (error) {
      failureCallback(error as Error);
    }
  };

  return (
    <FullCalendar
      allDaySlot={false}
      buttonIcons={false}
      buttonText={{
        today: "Hoje",
        timeGridDay: "Dia",
        timeGridWeek: "Semana",
        dayGridMonth: "Mes",
      }}
      customButtons={{
        prevPeriod: {
          text: "Ant.",
          click: () => calendarRef.current?.getApi().prev(),
        },
        nextPeriod: {
          text: "Prox.",
          click: () => calendarRef.current?.getApi().next(),
        },
      }}
      contentHeight={isCompact ? 560 : 720}
      editable={false}
      eventClick={handleEventClick}
      events={loadEvents}
      expandRows
      headerToolbar={{
        left: isCompact ? "prevPeriod,nextPeriod" : "prevPeriod,nextPeriod today",
        center: "title",
        right: isCompact ? "timeGridDay,timeGridWeek" : "timeGridDay,timeGridWeek,dayGridMonth",
      }}
      initialView="timeGridDay"
      locale={ptBrLocale}
      nextDayThreshold="00:00:00"
      plugins={[dayGridPlugin, timeGridPlugin, interactionPlugin]}
      ref={calendarRef}
      select={handleSelect}
      selectable
      selectMirror
      slotDuration="01:00:00"
      slotLabelFormat={{
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      }}
      slotMinTime={isCompact ? "08:00:00" : "07:00:00"}
      slotMaxTime={isCompact ? "21:00:00" : "22:00:00"}
      titleFormat={{ day: "2-digit", month: "long", year: "numeric" }}
    />
  );
}
