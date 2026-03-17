"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import FullCalendar from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/daygrid";
import interactionPlugin from "@fullcalendar/interaction";
import timeGridPlugin from "@fullcalendar/timegrid";
import ptBrLocale from "@fullcalendar/core/locales/pt-br";
import { useRouter } from "next/navigation";
import type { DateSelectArg, EventClickArg, EventSourceFuncArg, EventMountArg } from "@fullcalendar/core";

import { RescheduleDialog } from "@/components/forms/reschedule-dialog";

type CalendarEvent = {
  id: string;
  patientId: string;
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
  onRefetch?: (refetch: () => void) => void;
};

export function AppointmentsCalendar({ onSlotSelect, onRefetch }: AppointmentsCalendarProps) {
  const router = useRouter();
  const calendarRef = useRef<FullCalendar | null>(null);
  const [isCompact, setIsCompact] = useState(false);

  const refetchEvents = useCallback(() => {
    calendarRef.current?.getApi().refetchEvents();
  }, []);

  useEffect(() => {
    if (onRefetch) {
      onRefetch(refetchEvents);
    }
  }, [onRefetch, refetchEvents]);

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

  const [contextMenuPos, setContextMenuPos] = useState<{ x: number; y: number } | null>(null);
  const [contextEvent, setContextEvent] = useState<CalendarEvent | null>(null);
  const [rescheduleData, setRescheduleData] = useState<{ id: string; patientId: string; startsAt: string; endsAt: string } | null>(null);

  const handleEventMount = (mountInfo: EventMountArg) => {
    mountInfo.el.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      
      const evt = mountInfo.event;
      setContextEvent({
        id: evt.id,
        patientId: evt.extendedProps.patientId,
        title: evt.title,
        start: evt.startStr,
        end: evt.endStr,
        status: evt.extendedProps.status,
        confirmationStatus: evt.extendedProps.confirmationStatus,
      });
      // Slight delay to ensure it doesn't immediately close if there's a global click listener
      setTimeout(() => {
        setContextMenuPos({ x: e.clientX, y: e.clientY });
      }, 0);
    });
  };

  useEffect(() => {
    const handleClickOutside = () => {
      if (contextMenuPos) {
        setContextMenuPos(null);
        setContextEvent(null);
      }
    };
    
    document.addEventListener("click", handleClickOutside);
    document.addEventListener("contextmenu", handleClickOutside);
    
    return () => {
      document.removeEventListener("click", handleClickOutside);
      document.removeEventListener("contextmenu", handleClickOutside);
    };
  }, [contextMenuPos]);

  const handleRescheduleFinish = () => {
    refetchEvents();
    setRescheduleData(null);
  };

  return (
    <div 
      className="w-full relative"
      onContextMenu={(e) => {
        // Clear context menu if clicked outside a specific event
        const target = e.target as HTMLElement;
        if (!target.closest('.fc-event')) {
          setContextEvent(null);
          setContextMenuPos(null);
        }
      }}
    >
      <RescheduleDialog 
        appointmentId={rescheduleData?.id || null}
        patientId={rescheduleData?.patientId || ""}
        currentStartsAt={rescheduleData?.startsAt || ""}
        currentEndsAt={rescheduleData?.endsAt || ""}
        afterSuccess={handleRescheduleFinish}
        onOpenChange={(open) => !open && setRescheduleData(null)}
      />

      {contextMenuPos && contextEvent && typeof document !== "undefined" ? createPortal(
        <div 
          className="fixed z-[9999] min-w-[12rem] overflow-hidden rounded-md border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950 p-1 text-slate-950 dark:text-slate-50 shadow-lg animate-in fade-in-80 zoom-in-95"
          style={{ left: contextMenuPos.x, top: contextMenuPos.y }}
        >
          <div 
            className="cursor-pointer relative flex select-none items-center rounded-sm px-2 py-1.5 text-sm outline-none hover:bg-slate-100 dark:hover:bg-slate-800"
            onClick={(e) => {
              e.stopPropagation();
              router.push(`/appointments/${contextEvent.id}`);
              setContextEvent(null);
              setContextMenuPos(null);
            }}
          >
            Abrir detalhes
          </div>
          <div 
            className="cursor-pointer relative flex select-none items-center rounded-sm px-2 py-1.5 text-sm outline-none hover:bg-slate-100 dark:hover:bg-slate-800"
            onClick={(e) => {
              e.stopPropagation();
              setRescheduleData({
                id: contextEvent.id,
                patientId: contextEvent.patientId,
                startsAt: contextEvent.start,
                endsAt: contextEvent.end,
              });
              setContextEvent(null);
              setContextMenuPos(null);
            }}
          >
            Reagendar
          </div>
        </div>,
        document.body
      ) : null}

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
        eventDidMount={handleEventMount}
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
        scrollTime="06:00:00"
        select={handleSelect}
        selectable
        selectMirror
        slotDuration="01:00:00"
        slotLabelFormat={{
          hour: "2-digit",
          minute: "2-digit",
          hour12: false,
        }}
        slotMinTime="06:00:00"
        slotMaxTime={isCompact ? "21:00:00" : "22:00:00"}
        titleFormat={{ day: "2-digit", month: "long", year: "numeric" }}
      />
    </div>
  );
}
