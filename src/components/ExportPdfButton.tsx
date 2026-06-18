import { useRef, useState } from "react";
import Button from "./ui/Button";
import { toast } from "./ui/Toast";
import { FileText } from "lucide-react";
import { downloadFile } from "../lib/utils";

interface Props {
  targetRef: React.RefObject<HTMLDivElement | null>;
  filename?: string;
}

export default function ExportPdfButton({ targetRef, filename = "relatorio.pdf" }: Props) {
  const [loading, setLoading] = useState(false);

  async function handleExport() {
    if (!targetRef.current) return;
    setLoading(true);
    try {
      const html2canvas = (await import("html2canvas")).default;
      const jsPDF = (await import("jspdf")).default;

      const canvas = await html2canvas(targetRef.current, {
        scale: 2,
        useCORS: true,
        backgroundColor: "#f5f4f0",
      });

      const imgData = canvas.toDataURL("image/png");
      const pdf = new jsPDF("p", "mm", "a4");
      const pdfWidth = pdf.internal.pageSize.getWidth();
      const pdfHeight = (canvas.height * pdfWidth) / canvas.width;

      pdf.addImage(imgData, "PNG", 0, 0, pdfWidth, pdfHeight);
      const blob = pdf.output("blob");
      await downloadFile(blob, filename);
      toast("PDF exportado.");
    } catch (e: any) {
      toast(e.message || "Erro ao exportar PDF", "error");
    } finally {
      setLoading(false);
    }
  }

  return (
    <Button variant="outline" size="sm" onClick={handleExport} disabled={loading}>
      <FileText className="h-4 w-4 mr-2" />{loading ? "Exportando..." : "Exportar PDF"}
    </Button>
  );
}
