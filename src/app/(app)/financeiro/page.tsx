import Link from "next/link";
import { PageHeader } from "@/components/shell/page-header";
import { MetricCard } from "@/components/shell/metric-card";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { getFinancialSummary, listPayments, listPendingPayments } from "@/features/payments/queries";
import { requireUser } from "@/lib/auth/session";
import {
  describeAppointmentTime,
  formatCurrencyBRL,
  getPaymentMethodLabel,
  getPaymentStatusLabel,
} from "@/lib/utils";

type FinanceiroPageProps = {
  searchParams?: Promise<{ status?: string }>;
};

export default async function FinanceiroPage({ searchParams }: FinanceiroPageProps) {
  const user = await requireUser();
  const params = (await searchParams) ?? {};
  const statusFilter = params.status === "pending" ? "pending" : "all";
  const [payments, pendingPayments, summary] = await Promise.all([
    listPayments(user.id),
    listPendingPayments(user.id),
    getFinancialSummary(user.id),
  ]);
  const filteredPayments = statusFilter === "pending" ? pendingPayments : payments;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Financeiro"
        title="Controle manual de pagamentos"
        description="Acompanhe pendencias, recebimentos e detalhes por atendimento com uma trilha simples e objetiva."
      />

      <div className="grid gap-4 md:grid-cols-2">
        <MetricCard
          hint="Recebimentos confirmados no mes atual."
          label="Valores recebidos"
          value={formatCurrencyBRL(summary.paidCents)}
        />
        <Link href="/financeiro?status=pending">
          <MetricCard
            hint="Clique para ver apenas os atendimentos ainda nao pagos."
            label="Valores pendentes"
            value={formatCurrencyBRL(summary.pendingCents)}
          />
        </Link>
      </div>

      <Card>
        <CardHeader>
          <CardTitle id="pending-payments">
            {statusFilter === "pending" ? "Pagamentos pendentes" : "Pagamentos por atendimento"}
          </CardTitle>
          <CardDescription>
            {statusFilter === "pending"
              ? "Lista filtrada com os atendimentos concluidos que ainda aguardam pagamento."
              : "Visao consolidada de status, valor recebido e forma de pagamento."}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Paciente</TableHead>
                  <TableHead>Atendimento</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Metodo</TableHead>
                  <TableHead>Recebido</TableHead>
                  <TableHead className="text-right">Detalhe</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredPayments.map((payment) => (
                  <TableRow key={payment.id}>
                    <TableCell className="font-semibold text-slate-900">{payment.patientName}</TableCell>
                    <TableCell>{describeAppointmentTime(payment.startsAt)}</TableCell>
                    <TableCell>
                      <Badge variant={payment.status === "paid" ? "success" : payment.status === "pending" ? "warning" : "secondary"}>
                        {getPaymentStatusLabel(payment.status)}
                      </Badge>
                    </TableCell>
                    <TableCell>{getPaymentMethodLabel(payment.method)}</TableCell>
                    <TableCell>{formatCurrencyBRL(payment.amountReceivedCents || payment.sessionPriceCents)}</TableCell>
                    <TableCell className="text-right">
                      <Link className="font-semibold text-primary" href={`/appointments/${payment.appointmentId}`}>
                        Abrir
                      </Link>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
