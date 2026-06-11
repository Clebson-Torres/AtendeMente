import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "danger" | "ghost";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
}

const base = "inline-flex items-center justify-center rounded-lg px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50";

const variants: Record<Variant, string> = {
  primary: "bg-blue-600 text-white hover:bg-blue-700",
  secondary: "bg-gray-100 text-gray-700 hover:bg-gray-200",
  danger: "bg-red-600 text-white hover:bg-red-700",
  ghost: "text-gray-600 hover:text-gray-900 hover:bg-gray-100",
};

export default function Button({ variant = "primary", className = "", ...props }: Props) {
  return <button className={`${base} ${variants[variant]} ${className}`} {...props} />;
}
