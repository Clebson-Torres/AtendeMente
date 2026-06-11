const statusStyles: Record<string, string> = {
  active: "bg-green-100 text-green-800",
  inactive: "bg-gray-100 text-gray-600",
  confirmed: "bg-blue-100 text-blue-800",
  pending: "bg-yellow-100 text-yellow-800",
  cancelled: "bg-red-100 text-red-800",
  completed: "bg-green-100 text-green-800",
  paid: "bg-green-100 text-green-800",
  unpaid: "bg-yellow-100 text-yellow-800",
  partial: "bg-orange-100 text-orange-800",
};

interface Props {
  status: string;
}

export default function StatusBadge({ status }: Props) {
  const style = statusStyles[status.toLowerCase()] || "bg-gray-100 text-gray-600";
  return (
    <span className={`inline-block px-2 py-0.5 rounded-full text-xs font-medium ${style}`}>
      {status}
    </span>
  );
}
