import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** Resultado de um salvamento: onde foi parar, ou por que não foi. */
export type ResultadoSalvar =
  | { status: "salvo"; caminho: string }
  | { status: "cancelado" }
  | { status: "baixado" };

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function blobParaBase64(blob: Blob): Promise<string> {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  // Em pedaços: `String.fromCharCode(...bytes)` estoura a pilha com um backup
  // de alguns MB, e backup é exatamente o caso de uso aqui.
  let binario = "";
  const PEDACO = 0x8000;
  for (let i = 0; i < bytes.length; i += PEDACO) {
    binario += String.fromCharCode(...bytes.subarray(i, i + PEDACO));
  }
  return btoa(binario);
}

/**
 * Salva um arquivo, perguntando onde.
 *
 * No app instalado usa o diálogo nativo do sistema e devolve o caminho gravado.
 * Antes isto tentava `showSaveFilePicker` e caía num `<a download>`: a primeira só
 * existe em navegadores Chromium, então no WebKitGTK do Linux e no WKWebView do
 * macOS o arquivo era gravado sem perguntar nada e sem dizer onde — enquanto a tela
 * anunciava sucesso. Para a chave de recuperação isso significava perder o único
 * caminho de volta à conta.
 */
export async function downloadFile(
  blob: Blob,
  defaultName: string,
): Promise<ResultadoSalvar> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      const caminho = await invoke<string>("cmd_save_file", {
        filename: defaultName,
        contentBase64: await blobParaBase64(blob),
      });
      return { status: "salvo", caminho };
    } catch (e: any) {
      if (String(e?.message ?? e).includes("cancelado")) {
        return { status: "cancelado" };
      }
      throw e;
    }
  }

  // Navegador: o Chromium tem seletor de destino; os demais baixam na pasta padrão.
  try {
    const handle = await (window as any).showSaveFilePicker?.({
      suggestedName: defaultName,
    });
    if (handle) {
      const writable = await handle.createWritable();
      await writable.write(blob);
      await writable.close();
      return { status: "salvo", caminho: handle.name ?? defaultName };
    }
  } catch {
    // cancelado ou API ausente — cai no link
  }

  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = defaultName;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  return { status: "baixado" };
}

/** Mensagem de toast para o resultado de um salvamento. */
export function mensagemDeSalvamento(r: ResultadoSalvar, oQue: string): string | null {
  if (r.status === "cancelado") return null;
  if (r.status === "salvo") return `${oQue} salvo em ${r.caminho}`;
  return `${oQue} baixado para a pasta de downloads.`;
}
