import { cn } from "../../lib/utils";
import type { ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";

export interface Column<T> {
  key: string;
  header: string;
  render?: (item: T) => ReactNode;
  className?: string;
}

interface Props<T> {
  columns: Column<T>[];
  data: T[];
  keyExtractor: (item: T) => string;
  onRowClick?: (item: T) => void;
  emptyMessage?: string;
  page?: number;
  total?: number;
  perPage?: number;
  onPageChange?: (page: number) => void;
}

export default function DataTable<T>({ columns, data, keyExtractor, onRowClick, emptyMessage, page, total, perPage, onPageChange }: Props<T>) {
  if (data.length === 0) {
    return (
      <div className="text-center py-12 text-muted-foreground text-sm">
        {emptyMessage || "Nenhum registro encontrado."}
      </div>
    );
  }

  const totalPages = total !== undefined && perPage ? Math.max(1, Math.ceil(total / perPage)) : 1;

  return (
    <div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border">
              {columns.map((col) => (
                <th key={col.key} className={cn("text-left px-4 py-3 font-medium text-muted-foreground", col.className)}>
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.map((item) => (
              <tr
                key={keyExtractor(item)}
                onClick={() => onRowClick?.(item)}
                className={cn(
                  "border-b border-border/50 transition-colors",
                  onRowClick ? "cursor-pointer hover:bg-muted/50" : "",
                )}
              >
                {columns.map((col) => (
                  <td key={col.key} className={cn("px-4 py-3 text-foreground", col.className)}>
                    {col.render ? col.render(item) : (item as any)[col.key] ?? "-"}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {onPageChange && totalPages > 1 && (
        <div className="flex items-center justify-between px-4 py-3 border-t border-border">
          <span className="text-sm text-muted-foreground">
            {((page ?? 1) - 1) * (perPage ?? 50) + 1}–{Math.min((page ?? 1) * (perPage ?? 50), total ?? 0)} de {total}
          </span>
          <div className="flex items-center gap-1">
            <button
              onClick={() => onPageChange((page ?? 1) - 1)}
              disabled={(page ?? 1) <= 1}
              className="p-1 rounded hover:bg-muted disabled:opacity-30 disabled:cursor-not-allowed"
            >
              <ChevronLeft className="h-4 w-4" />
            </button>
            {Array.from({ length: totalPages }, (_, i) => i + 1).map((p) => (
              <button
                key={p}
                onClick={() => onPageChange(p)}
                className={cn(
                  "px-2 py-1 text-sm rounded",
                  p === (page ?? 1) ? "bg-primary text-primary-foreground" : "hover:bg-muted"
                )}
              >
                {p}
              </button>
            ))}
            <button
              onClick={() => onPageChange((page ?? 1) + 1)}
              disabled={(page ?? 1) >= totalPages}
              className="p-1 rounded hover:bg-muted disabled:opacity-30 disabled:cursor-not-allowed"
            >
              <ChevronRight className="h-4 w-4" />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
