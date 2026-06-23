import { useEffect, useRef, useState } from "react";
import QRCode from "qrcode";
import { Smartphone, Wifi, Copy, Check, WifiOff, Settings as SettingsIcon } from "lucide-react";
import { toast } from "../components/ui/Toast";
import Skeleton from "../components/ui/Skeleton";
import { api } from "../lib/api";
import { useNavigate } from "react-router-dom";

interface NetworkInfo {
  ipv4: string[];
  ipv6: string[];
  port: number;
}

export default function NetworkInfoPage() {
  const navigate = useNavigate();
  const [info, setInfo] = useState<NetworkInfo | null>(null);
  const [mobileEnabled, setMobileEnabled] = useState(true);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);
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
    if (!info || !canvasRef.current || info.ipv4.length === 0) return;
    const url = `http://${info.ipv4[0]}:${info.port}`;
    QRCode.toCanvas(canvasRef.current, url, {
      width: 280,
      margin: 2,
      color: { dark: "#1e293b", light: "#ffffff" },
    });
  }, [info]);

  async function handleCopy() {
    if (!info || info.ipv4.length === 0) return;
    const url = `http://${info.ipv4[0]}:${info.port}`;
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toast("Erro ao copiar", "error");
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
      ) : !info || info.ipv4.length === 0 ? (
        <div className="text-center py-12 text-slate-400">
          <Wifi className="h-12 w-12 mx-auto mb-3" />
          <p>Nenhum endereço de rede encontrado.</p>
          <p className="text-sm">Verifique se o servidor está rodando na rede local.</p>
        </div>
      ) : (
        <div className="bg-white rounded-2xl shadow-sm border border-slate-200 p-6 space-y-6">
          <div className="flex justify-center">
            <canvas ref={canvasRef} className="rounded-xl" />
          </div>

          <div className="text-center space-y-1">
            <p className="text-sm text-slate-500">Escaneie o QR code com seu celular</p>
            <p className="text-xs text-slate-400">Conectado à mesma rede Wi-Fi</p>
          </div>

          <div className="bg-slate-50 rounded-xl p-4 space-y-2">
            <p className="text-xs font-medium text-slate-500 uppercase tracking-wide">Endereços</p>
            {info.ipv4.map((ip) => (
              <div key={ip} className="flex items-center justify-between">
                <code className="text-sm text-slate-700 font-mono">http://{ip}:{info.port}</code>
                <button
                  onClick={handleCopy}
                  className="p-1.5 rounded-lg hover:bg-slate-200 transition-colors text-slate-400 hover:text-slate-600"
                  title="Copiar URL"
                >
                  {copied ? <Check className="h-4 w-4 text-green-600" /> : <Copy className="h-4 w-4" />}
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
