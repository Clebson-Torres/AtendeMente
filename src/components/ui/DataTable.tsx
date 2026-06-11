import type { ReactNode } from "react";

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
}

export default function DataTable<T>({ columns, data, keyExtractor, onRowClick, emptyMessage }: Props<T>) {
  if (data.length === 0) {
    return (
      <div className="text-center py-12 text-gray-400 text-sm">
        {emptyMessage || "Nenhum registro encontrado."}
      </div>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-gray-200">
            {columns.map((col) => (
              <th key={col.key} className={`text-left px-3 py-3 font-medium text-gray-500 ${col.className || ""}`}>
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
              className={`border-b border-gray-100 hover:bg-gray-50 transition-colors ${onRowClick ? "cursor-pointer" : ""}`}
            >
              {columns.map((col) => (
                <td key={col.key} className={`px-3 py-3 text-gray-700 ${col.className || ""}`}>
                  {col.render ? col.render(item) : (item as any)[col.key] ?? "-"}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
