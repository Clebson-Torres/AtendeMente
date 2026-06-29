import { useEffect, useRef, useState } from "react";
import QRCode from "qrcode";
import {
  Smartphone,
  Wifi,
  WifiOff,
  Copy,
  Check,
  Settings as SettingsIcon,
  CheckCircle,
  AlertTriangle,
  Circle,
} from "lucide-react";
import { toast } from "../components/ui/Toast";
import Skeleton from "../components/ui/Skeleton";
import { api } from "../lib/api";
import { useNavigate } from "react-router-dom";

interface NetworkInterface {
  name: string;
  ip: string;
  is_recommended: boolean;
  is_vpn: boolean;
  interface_type: string;
}

interface NetworkInfo {
  interfaces: NetworkInterface[];
  port: number;
  has_vpn: boolean;
  server_bound_to: string;
}

export default function NetworkInfoPage() {
  const navigate = useNavigate();
  const [info, setInfo] = useState<NetworkInfo | null>(null);
  const [mobileEnabled, setMobileEnabled] = useState(true);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);
  const [selectedIp, setSelectedIp] = useState("");
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    Promise.all([
      api.settings.getNetworkInfo(),
      api.settings.getMobileAccess(),
    ])
      .then(([netData, settingsData]) => {
        setInfo(netData);
        setMobileEnabled(settingsData.enabled);
        setLoading(false);
      })
      .catch(() => {
        setLoading(false);
      });
  }, []);

  useEffect(() => {
    if (info && info.interfaces.length > 0 && !selectedIp) {
      const recommended = info.interfaces.find((i) => i.is_recommended);
      setSelectedIp(recommended ? recommended.ip : info.interfaces[0].ip);
    }
  }, [info, selectedIp]);

  useEffect(() => {
    if (!info || !canvasRef.current || !selectedIp) return;
    const url = `http://${selectedIp}:${info.port}`;
    QRCode.toCanvas(canvasRef.current, url, {
      width: 280,
      margin: 2,
      color: { dark: "#1e293b", light: "#ffffff" },
    });
  }, [info, selectedIp]);

  async function handleCopy() {
    if (!selectedIp || !info) return;
    const url = `http://${selectedIp}:${info.port}`;
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toast("Erro ao copiar", "error");
    }
  }

  function getInterfaceIcon(type: string) {
    switch (type) {
      case "wifi":
        return <Wifi className="h-4 w-4" />;
      case "ethernet":
        return <span className="text-xs">🔗</span>;
      case "vpn":
        return <AlertTriangle className="h-4 w-4" />;
      default:
        return <Circle className="h-4 w-4" />;
    }
  }

  if (loading) {
    return (
      <div className="p-6 max-w-xl mx-auto space-y-4">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-72 w-full" />
      </div>
    );
  }

  return (
    <div className="p-6 max-w-xl mx-auto">
      <div className="flex items-center gap-3 mb-6">
        <Smartphone className="h-6 w-6 text-teal-600" />
        <h1 className="text-xl font-semibold text-slate-800">Acesso Mobile</h1>
      </div>

      {!mobileEnabled ? (
        <div className="text-center py-12 text-slate-400">
          <WifiOff className="h-12 w-12 mx-auto mb-3" />
          <p>Acesso mobile está desativado.</p>
          <p className="text-sm mb-4">Ative em Configurações para acessar pelo celular.</p>
          <button
            onClick={() => navigate("/settings")}
            className="inline-flex items-center gap-2 text-sm text-primary hover:text-primary/80 transition-colors"
          >
            <SettingsIcon className="h-4 w-4" /> Ir para Configurações
          </button>
        </div>
      ) : !info || info.interfaces.length === 0 ? (
        <div className="text-center py-12 text-slate-400">
          <Wifi className="h-12 w-12 mx-auto mb-3" />
          <p>Nenhuma interface de rede encontrada.</p>
          <p className="text-sm">Verifique se o servidor está rodando na rede local.</p>
        </div>
      ) : (
        <div className="space-y-4">
          {/* QR Code */}
          <div className="bg-white rounded-2xl shadow-sm border border-slate-200 p-6 space-y-4">
            <div className="flex justify-center">
              <canvas ref={canvasRef} className="rounded-xl" />
            </div>
            <div className="text-center space-y-1">
              <p className="text-sm text-slate-500">Escaneie o QR code com seu celular</p>
              <p className="text-xs text-slate-400">Conectado à mesma rede Wi-Fi</p>
            </div>
          </div>

          {/* Interface selector */}
          <div className="bg-white rounded-2xl shadow-sm border border-slate-200 p-4 space-y-2">
            <p className="text-xs font-medium text-slate-500 uppercase tracking-wide px-1">
              Interfaces disponíveis
            </p>
            {info.interfaces.map((iface) => (
              <button
                key={iface.ip}
                onClick={() => setSelectedIp(iface.ip)}
                className={`w-full flex items-center justify-between p-3 rounded-lg border transition-colors cursor-pointer ${
                  selectedIp === iface.ip
                    ? "border-teal-600 bg-teal-50"
                    : "border-slate-200 bg-white hover:border-slate-300"
                }`}
              >
                <div className="flex items-center gap-2">
                  {iface.is_recommended && (
                    <CheckCircle className="h-4 w-4 text-green-600" />
                  )}
                  {iface.is_vpn && !iface.is_recommended && (
                    <AlertTriangle className="h-4 w-4 text-amber-500" />
                  )}
                  {!iface.is_recommended && !iface.is_vpn && (
                    <span className="h-4 w-4 flex items-center justify-center text-slate-300">
                      {getInterfaceIcon(iface.interface_type)}
                    </span>
                  )}
                  <span className="text-sm font-medium text-slate-700">{iface.name}</span>
                  {iface.is_recommended && (
                    <span className="text-xs text-green-600 font-medium">recomendada</span>
                  )}
                </div>
                <code className="text-xs text-slate-500 font-mono">{iface.ip}</code>
              </button>
            ))}
          </div>

          {/* Direct link */}
          {selectedIp && (
            <div className="bg-white rounded-2xl shadow-sm border border-slate-200 p-4">
              <p className="text-xs font-medium text-slate-500 uppercase tracking-wide mb-2 px-1">
                Link direto
              </p>
              <div className="flex items-center justify-between bg-slate-50 rounded-lg p-3">
                <code className="text-sm text-slate-700 font-mono break-all">
                  http://{selectedIp}:{info.port}
                </code>
                <button
                  onClick={handleCopy}
                  className="p-1.5 rounded-lg hover:bg-slate-200 transition-colors text-slate-400 hover:text-slate-600 ml-2 flex-shrink-0"
                  title="Copiar URL"
                >
                  {copied ? <Check className="h-4 w-4 text-green-600" /> : <Copy className="h-4 w-4" />}
                </button>
              </div>
            </div>
          )}

          {/* Diagnostics */}
          <div className="bg-white rounded-2xl shadow-sm border border-slate-200 p-4 space-y-2">
            <p className="text-xs font-medium text-slate-500 uppercase tracking-wide px-1">
              Diagnóstico
            </p>
            <div className="space-y-1.5">
              <DiagnosticItem ok={true} text="Servidor iniciado" />
              <DiagnosticItem ok={info.server_bound_to === "0.0.0.0"} text="Porta aberta na rede" warning={info.server_bound_to !== "0.0.0.0"} />
              <DiagnosticItem ok={selectedIp !== ""} text="Interface detectada" />
              <DiagnosticItem ok={!info.has_vpn} warning={info.has_vpn} text="VPN ativa" />
            </div>
          </div>

          {info.server_bound_to === "127.0.0.1" && (
            <div className="bg-amber-50 border border-amber-200 rounded-2xl p-4 text-sm text-amber-700 space-y-1">
              <p className="font-medium">Servidor escutando apenas localmente (127.0.0.1)</p>
              <p>Ative o "Acesso Mobile" nas Configurações e reinicie o aplicativo para liberar conexões externas.</p>
            </div>
          )}

          {/* Tips */}
          <div className="bg-slate-50 rounded-2xl p-4 text-xs text-slate-500 space-y-1">
            <p className="font-medium text-slate-600">Dicas:</p>
            <ul className="list-disc list-inside space-y-0.5">
              <li>Conecte o celular na mesma rede do computador</li>
              <li>Verifique se o firewall permite conexões na porta {info.port}</li>
              {info.has_vpn && (
                <li className="text-amber-600">
                  VPN detectada. O celular pode não conseguir acessar este endereço.
                </li>
              )}
            </ul>
          </div>
        </div>
      )}
    </div>
  );
}

function DiagnosticItem({
  ok,
  warning,
  text,
}: {
  ok: boolean;
  warning?: boolean;
  text: string;
}) {
  return (
    <div className="flex items-center gap-2">
      {ok && !warning ? (
        <CheckCircle className="h-4 w-4 text-green-600" />
      ) : warning ? (
        <AlertTriangle className="h-4 w-4 text-amber-500" />
      ) : (
        <Circle className="h-4 w-4 text-slate-300" />
      )}
      <span className={`text-sm ${ok && !warning ? "text-slate-700" : warning ? "text-amber-600" : "text-slate-400"}`}>
        {text}
      </span>
    </div>
  );
}
