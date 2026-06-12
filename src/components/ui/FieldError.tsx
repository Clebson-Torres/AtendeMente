interface Props {
  message?: string;
}

export default function FieldError({ message }: Props) {
  if (!message) return null;
  return <p className="text-xs text-destructive mt-1">{message}</p>;
}
