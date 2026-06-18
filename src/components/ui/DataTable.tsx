import { cn } from "../../lib/utils";
import { useState, type ReactNode } from "react";
import { ChevronLeft, ChevronRight, ArrowUpDown, ArrowUp, ArrowDown } from "lucide-react";

export interface Column<T> {
  key: string;
  header: string;
  render?: (item: T) => ReactNode;
  className?: string;
  sortable?: boolean;
  sortKey?: string;
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
  const [sortColumn, setSortColumn] = useState<string | null>(null);
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");

  function handleSort(key: string) {
    if (sortColumn === key) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortColumn(key);
      setSortDir("asc");
    }
  }

  const sorted = [...data].sort((a, b) => {
    if (!sortColumn) return 0;
    const aVal = (a as any)[sortColumn];
    const bVal = (b as any)[sortColumn];
    if (aVal == null) return 1;
    if (bVal == null) return -1;
    const cmp = typeof aVal === "string" ? aVal.localeCompare(bVal) : aVal - bVal;
    return sortDir === "asc" ? cmp : -cmp;
  });
  if (data.length === 0) {
    return (
      <div className="text-center py-12 text-muted-foreground text-sm">
        {emptyMessage || "Nenhum registro encontrado."}
      </div>
    );
  }

  const totalPages = total !== undefined && perPage ? Math.max(1, Math.ceil(total / perPage)) : 1;

  function getPageNumbers(current: number, total: number): (number | "...")[] {
    if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1);
    const pages: (number | "...")[] = [];
    pages.push(1);
    if (current > 3) pages.push("...");
    const start = Math.max(2, current - 1);
    const end = Math.min(total - 1, current + 1);
    for (let i = start; i <= end; i++) pages.push(i);
    if (current < total - 2) pages.push("...");
    pages.push(total);
    return pages;
  }

  return (
    <div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border">
              {columns.map((col) => (
                <th key={col.key}
                  className={cn(
                    "text-left px-4 py-3 font-medium text-muted-foreground",
                    col.sortable && "cursor-pointer select-none hover:text-foreground transition-colors",
                    col.className,
                  )}
                  onClick={() => col.sortable && handleSort(col.sortKey || col.key)}
                >
                  <span className="inline-flex items-center gap-1">
                    {col.header}
                    {col.sortable && (
                      sortColumn === (col.sortKey || col.key)
                        ? (sortDir === "asc" ? <ArrowUp className="h-3 w-3" /> : <ArrowDown className="h-3 w-3" />)
                        : <ArrowUpDown className="h-3 w-3 opacity-40" />
                    )}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {sorted.map((item) => (
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
              aria-label="Página anterior"
              className="p-1 rounded hover:bg-muted disabled:opacity-30 disabled:cursor-not-allowed"
            >
              <ChevronLeft className="h-4 w-4" />
            </button>
            {getPageNumbers(page ?? 1, totalPages).map((p, i) =>
              p === "..." ? (
                <span key={`ellipsis-${i}`} className="px-2 py-1 text-sm text-muted-foreground">...</span>
              ) : (
                <button
                  key={p}
                  onClick={() => onPageChange(p as number)}
                  className={cn(
                    "px-2 py-1 text-sm rounded",
                    p === (page ?? 1) ? "bg-primary text-primary-foreground" : "hover:bg-muted"
                  )}
                >
                  {p}
                </button>
              )
            )}
            <button
              onClick={() => onPageChange((page ?? 1) + 1)}
              disabled={(page ?? 1) >= totalPages}
              aria-label="Próxima página"
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
