#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Gera o relatório de auditoria de segurança do AtendeMente em PDF.

Uso:
    ./.venv/bin/python gerar_relatorio.py

Os dados dos achados vivem em `achados.py`; este arquivo cuida só do layout.
Para reemitir o relatório depois de uma correção, edite `achados.py` e rode
este script de novo.
"""

import io
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_JUSTIFY, TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import cm, mm
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    BaseDocTemplate, CondPageBreak, Frame, HRFlowable, KeepTogether, PageBreak,
    PageTemplate, Paragraph, Preformatted, Spacer, Table, TableStyle,
)

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)

import achados as D  # noqa: E402

SAIDA = os.path.join(AQUI, "relatorio-auditoria-seguranca.pdf")
TITULO = f"Relatório de Auditoria de Segurança — {D.PROJETO}"

MARGEM = 2 * cm
LARGURA_UTIL = A4[0] - 2 * MARGEM

TINTA = colors.HexColor("#0F172A")
TINTA_SUAVE = colors.HexColor("#475569")
LINHA = colors.HexColor("#E2E8F0")
FUNDO_SUAVE = colors.HexColor("#F8FAFC")
FUNDO_CODIGO = colors.HexColor("#F1F5F9")


# ─── Fontes ──────────────────────────────────────────────────────────────────

def registrar_fontes():
    """Usa DejaVu se disponível (melhor cobertura de acentos e do caractere ↔).

    Cai para as fontes base do ReportLab se não achar — o relatório continua
    legível, apenas com tipografia diferente.
    """
    candidatos = [
        ("/usr/share/fonts/dejavu-sans-fonts", "DejaVuSans"),
        ("/usr/share/fonts/truetype/dejavu", "DejaVuSans"),
        ("/usr/share/fonts/TTF", "DejaVuSans"),
    ]
    for pasta, base in candidatos:
        regular = os.path.join(pasta, f"{base}.ttf")
        bold = os.path.join(pasta, f"{base}-Bold.ttf")
        if os.path.exists(regular) and os.path.exists(bold):
            pdfmetrics.registerFont(TTFont("Corpo", regular))
            pdfmetrics.registerFont(TTFont("Corpo-Bold", bold))
            obliquo = os.path.join(pasta, f"{base}-Oblique.ttf")
            pdfmetrics.registerFont(
                TTFont("Corpo-Italic", obliquo if os.path.exists(obliquo) else regular)
            )
            pdfmetrics.registerFontFamily(
                "Corpo", normal="Corpo", bold="Corpo-Bold",
                italic="Corpo-Italic", boldItalic="Corpo-Bold",
            )
            mono_dir = pasta.replace("dejavu-sans-fonts", "dejavu-sans-mono-fonts")
            mono = os.path.join(mono_dir, "DejaVuSansMono.ttf")
            if not os.path.exists(mono):
                mono = os.path.join(pasta, "DejaVuSansMono.ttf")
            if os.path.exists(mono):
                pdfmetrics.registerFont(TTFont("Mono", mono))
                return "Corpo", "Corpo-Bold", "Mono"
            return "Corpo", "Corpo-Bold", "Courier"
    return "Helvetica", "Helvetica-Bold", "Courier"


F_CORPO, F_BOLD, F_MONO = registrar_fontes()


# ─── Estilos ─────────────────────────────────────────────────────────────────

_base = getSampleStyleSheet()

E = {}
E["capa_kicker"] = ParagraphStyle(
    "capa_kicker", parent=_base["Normal"], fontName=F_BOLD, fontSize=10.5,
    textColor=colors.HexColor("#059669"), alignment=TA_CENTER, spaceAfter=6,
    tracking=1,
)
E["capa_titulo"] = ParagraphStyle(
    "capa_titulo", parent=_base["Title"], fontName=F_BOLD, fontSize=25, leading=31,
    textColor=TINTA, alignment=TA_CENTER, spaceAfter=4,
)
E["capa_sub"] = ParagraphStyle(
    "capa_sub", parent=_base["Normal"], fontName=F_CORPO, fontSize=11.5, leading=17,
    textColor=TINTA_SUAVE, alignment=TA_CENTER,
)
E["h1"] = ParagraphStyle(
    "h1", parent=_base["Heading1"], fontName=F_BOLD, fontSize=17, leading=22,
    textColor=TINTA, spaceBefore=2, spaceAfter=3,
)
E["h2"] = ParagraphStyle(
    "h2", parent=_base["Heading2"], fontName=F_BOLD, fontSize=12.5, leading=17,
    textColor=TINTA, spaceBefore=13, spaceAfter=5,
)
E["h3"] = ParagraphStyle(
    "h3", parent=_base["Heading3"], fontName=F_BOLD, fontSize=10.5, leading=14,
    textColor=TINTA, spaceBefore=8, spaceAfter=3,
)
E["corpo"] = ParagraphStyle(
    "corpo", parent=_base["Normal"], fontName=F_CORPO, fontSize=9.4, leading=14.2,
    textColor=TINTA, alignment=TA_JUSTIFY, spaceAfter=6,
)
E["corpo_esq"] = ParagraphStyle("corpo_esq", parent=E["corpo"], alignment=TA_LEFT)
E["nota"] = ParagraphStyle(
    "nota", parent=E["corpo"], fontSize=8.6, leading=12.6, textColor=TINTA_SUAVE,
)
E["celula"] = ParagraphStyle(
    "celula", parent=_base["Normal"], fontName=F_CORPO, fontSize=8.4, leading=12,
    textColor=TINTA, alignment=TA_LEFT,
)
E["celula_bold"] = ParagraphStyle("celula_bold", parent=E["celula"], fontName=F_BOLD)
E["celula_mono"] = ParagraphStyle(
    "celula_mono", parent=E["celula"], fontName=F_MONO, fontSize=6.9, leading=9.8,
    textColor=colors.HexColor("#1E293B"),
)
E["cabecalho_tab"] = ParagraphStyle(
    "cabecalho_tab", parent=E["celula"], fontName=F_BOLD, fontSize=8.4,
    textColor=colors.white,
)
E["codigo"] = ParagraphStyle(
    "codigo", parent=_base["Code"], fontName=F_MONO, fontSize=7.0, leading=9.6,
    textColor=colors.HexColor("#0F172A"), leftIndent=0, spaceAfter=0, spaceBefore=0,
)
E["issue"] = ParagraphStyle(
    "issue", parent=E["codigo"], fontSize=7.3, leading=10.2,
)
E["marcador"] = ParagraphStyle(
    "marcador", parent=E["corpo"], leftIndent=13, bulletIndent=3, spaceAfter=3.5,
    alignment=TA_LEFT,
)


def cor(sev):
    return colors.HexColor(D.PALETA[sev])


# ─── Componentes ─────────────────────────────────────────────────────────────

def chip(texto, cor_fundo, largura=None):
    """Etiqueta colorida com texto branco, usada para severidade e prioridade."""
    p = Paragraph(
        f'<font color="white"><b>{texto}</b></font>',
        ParagraphStyle("chip", parent=E["celula"], fontName=F_BOLD, fontSize=7.6,
                       leading=10, alignment=TA_CENTER),
    )
    t = Table([[p]], colWidths=[largura or 1.75 * cm], rowHeights=[0.46 * cm])
    t.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), cor_fundo),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("LEFTPADDING", (0, 0), (-1, -1), 2),
        ("RIGHTPADDING", (0, 0), (-1, -1), 2),
        ("TOPPADDING", (0, 0), (-1, -1), 1),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 1),
        ("ROUNDEDCORNERS", [3, 3, 3, 3]),
    ]))
    return t


def _fatiar(texto, estilo, por_fatia=4, largura_max=118):
    """Quebra o texto em fatias de poucas linhas, uma por linha de tabela.

    Existe por causa da paginação: um único Preformatted não se divide entre
    páginas, então uma issue longa estourava o frame; e uma Table de linha única
    tampouco divide. Fatiado, o fundo é repintado em cada página.
    """
    linhas = [ln.rstrip() or " " for ln in texto.strip("\n").split("\n")]
    fatias = []
    for i in range(0, len(linhas), por_fatia):
        pedaco = "\n".join(linhas[i:i + por_fatia])
        fatias.append([Preformatted(pedaco, estilo, maxLineLength=largura_max,
                                    splitChars=" ")])
    return fatias or [[Preformatted("", estilo)]]


def bloco_codigo(texto, largura=LARGURA_UTIL):
    """Trecho de código com fundo cinza e barra de acento à esquerda."""
    fatias = _fatiar(texto, E["codigo"])
    t = Table(fatias, colWidths=[largura], splitByRow=1)
    t.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), FUNDO_CODIGO),
        ("LINEBEFORE", (0, 0), (0, -1), 2.4, colors.HexColor("#94A3B8")),
        ("LEFTPADDING", (0, 0), (-1, -1), 8),
        ("RIGHTPADDING", (0, 0), (-1, -1), 6),
        ("TOPPADDING", (0, 0), (-1, -1), 0),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 0),
        ("TOPPADDING", (0, 0), (-1, 0), 6),
        ("BOTTOMPADDING", (0, -1), (-1, -1), 6),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
    ]))
    return t


def caixa(titulo, corpo, cor_borda, largura=LARGURA_UTIL):
    """Bloco rotulado com barra colorida à esquerda."""
    conteudo = [
        Paragraph(f'<font color="#{cor_borda.hexval()[2:]}"><b>{titulo}</b></font>',
                  ParagraphStyle("cx_t", parent=E["celula"], fontName=F_BOLD, fontSize=8.2,
                                 spaceAfter=3)),
        Paragraph(corpo, ParagraphStyle("cx_c", parent=E["celula"], alignment=TA_JUSTIFY,
                                        leading=12.4)),
    ]
    t = Table([[conteudo]], colWidths=[largura])
    t.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), FUNDO_SUAVE),
        ("LINEBEFORE", (0, 0), (0, -1), 2.4, cor_borda),
        ("LEFTPADDING", (0, 0), (-1, -1), 8),
        ("RIGHTPADDING", (0, 0), (-1, -1), 8),
        ("TOPPADDING", (0, 0), (-1, -1), 6),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
    ]))
    return t


def regua(cor_linha=LINHA, espessura=0.8, espaco_antes=4, espaco_depois=8):
    return HRFlowable(width="100%", thickness=espessura, color=cor_linha,
                      spaceBefore=espaco_antes, spaceAfter=espaco_depois)


# ─── Gráficos ────────────────────────────────────────────────────────────────

def _png(fig):
    buf = io.BytesIO()
    fig.savefig(buf, format="png", dpi=210, bbox_inches="tight",
                facecolor="white", edgecolor="none")
    plt.close(fig)
    buf.seek(0)
    return buf


def grafico_rosca(contagem):
    from reportlab.platypus import Image
    itens = [(s, n) for s, n in contagem.items() if n]
    rotulos = [f"{D.ROTULO_SEV[s]}  ({n})" for s, n in itens]
    valores = [n for _, n in itens]
    cores = [D.PALETA[s] for s, _ in itens]

    fig, ax = plt.subplots(figsize=(4.5, 3.3))
    wedges, _ = ax.pie(
        valores, colors=cores, startangle=90, counterclock=False,
        wedgeprops=dict(width=0.42, edgecolor="white", linewidth=2.2),
    )
    total = sum(valores)
    ax.text(0, 0.1, str(total), ha="center", va="center",
            fontsize=27, fontweight="bold", color="#0F172A")
    ax.text(0, -0.24, "achados", ha="center", va="center",
            fontsize=9.5, color="#64748B")
    ax.legend(wedges, rotulos, loc="center left", bbox_to_anchor=(1.0, 0.5),
              frameon=False, fontsize=9, handlelength=1.1, handleheight=1.1,
              labelspacing=0.72)
    ax.set_aspect("equal")
    return Image(_png(fig), width=9.3 * cm, height=6.2 * cm)


def grafico_barras(por_categoria):
    from reportlab.platypus import Image
    rotulos = list(por_categoria.keys())
    fig, ax = plt.subplots(figsize=(7.2, 3.5))

    ordem = ["critica", "alta", "media", "baixa", "informativa"]
    esquerda = [0] * len(rotulos)
    for sev in ordem:
        valores = [por_categoria[c].get(sev, 0) for c in rotulos]
        if not any(valores):
            continue
        ax.barh(rotulos, valores, left=esquerda, color=D.PALETA[sev],
                height=0.56, label=D.ROTULO_SEV[sev], edgecolor="white", linewidth=1.1)
        esquerda = [e + v for e, v in zip(esquerda, valores)]

    for i, total in enumerate(esquerda):
        if total:
            ax.text(total + 0.09, i, str(int(total)), va="center", ha="left",
                    fontsize=9.5, fontweight="bold", color="#0F172A")

    ax.invert_yaxis()
    ax.set_xlim(0, max(esquerda) + 0.85)
    ax.set_xlabel("Achados", fontsize=9, color="#475569", labelpad=7)
    ax.tick_params(axis="y", labelsize=9, colors="#0F172A", length=0)
    ax.tick_params(axis="x", labelsize=8.5, colors="#64748B", length=0)
    ax.xaxis.set_major_locator(matplotlib.ticker.MaxNLocator(integer=True))
    for lado in ("top", "right", "left"):
        ax.spines[lado].set_visible(False)
    ax.spines["bottom"].set_color("#CBD5E1")
    ax.grid(axis="x", color="#E2E8F0", linewidth=0.8)
    ax.set_axisbelow(True)
    ax.legend(loc="lower center", bbox_to_anchor=(0.5, 1.01), frameon=False,
              fontsize=8.5, ncol=5, handlelength=1.0, handleheight=1.0,
              columnspacing=1.4)
    return Image(_png(fig), width=16.4 * cm, height=8.0 * cm)


# ─── Cabeçalho e rodapé ──────────────────────────────────────────────────────

def decorar(canvas, doc):
    canvas.saveState()
    pagina = canvas.getPageNumber()
    if pagina > 1:
        canvas.setFont(F_CORPO, 7.6)
        canvas.setFillColor(TINTA_SUAVE)
        canvas.drawString(MARGEM, A4[1] - 1.28 * cm, TITULO)
        canvas.drawRightString(A4[0] - MARGEM, A4[1] - 1.28 * cm,
                               f"v{D.VERSAO_AUDITADA} · {D.COMMIT}")
        canvas.setStrokeColor(LINHA)
        canvas.setLineWidth(0.6)
        canvas.line(MARGEM, A4[1] - 1.48 * cm, A4[0] - MARGEM, A4[1] - 1.48 * cm)

        canvas.line(MARGEM, 1.42 * cm, A4[0] - MARGEM, 1.42 * cm)
        canvas.setFont(F_CORPO, 7.6)
        canvas.setFillColor(TINTA_SUAVE)
        canvas.drawString(MARGEM, 1.02 * cm, f"{D.PROJETO} · Auditoria de segurança · {D.DATA}")
        canvas.setFont(F_BOLD, 7.6)
        canvas.drawRightString(A4[0] - MARGEM, 1.02 * cm, f"Página {pagina}")
    canvas.restoreState()


# ─── Seções ──────────────────────────────────────────────────────────────────

def secao_capa():
    fluxo = [Spacer(1, 3.4 * cm)]

    faixa = Table([[""]], colWidths=[LARGURA_UTIL], rowHeights=[0.2 * cm])
    faixa.setStyle(TableStyle([("BACKGROUND", (0, 0), (-1, -1), colors.HexColor("#059669"))]))
    fluxo += [faixa, Spacer(1, 0.9 * cm)]

    fluxo.append(Paragraph("RELATÓRIO TÉCNICO", E["capa_kicker"]))
    fluxo.append(Paragraph(
        f"Relatório de Auditoria<br/>de Segurança<br/>"
        f'<font size="17" color="#475569">{D.PROJETO}</font>', E["capa_titulo"]))
    fluxo.append(Spacer(1, 0.7 * cm))
    fluxo.append(Paragraph(
        f"{D.DATA}<br/>Versão auditada <b>{D.VERSAO_AUDITADA}</b> · commit <b>{D.COMMIT}</b>",
        E["capa_sub"]))
    fluxo.append(Spacer(1, 1.5 * cm))

    fluxo.append(Paragraph("Escopo auditado", E["h2"]))
    fluxo.append(Paragraph(
        "Auditoria de código, estática e manual, do repositório completo na revisão indicada: "
        "backend Rust/Axum, frontend React, camada de persistência SQLite e migrations, "
        "configuração do Tauri, workflow de CI e o histórico do git. "
        "<b>Não</b> fazem parte do escopo: teste dinâmico contra uma instalação em execução, "
        "análise de dependências de terceiros (npm e crates), engenharia reversa dos binários "
        "publicados e revisão da infraestrutura de qualquer serviço externo.",
        E["corpo"]))

    fluxo.append(Paragraph("Stack detectada", E["h2"]))
    linhas = [[Paragraph("Camada", E["cabecalho_tab"]),
               Paragraph("Tecnologia", E["cabecalho_tab"])]]
    for camada, tec in D.STACK:
        linhas.append([Paragraph(f"<b>{camada}</b>", E["celula"]),
                       Paragraph(tec, E["celula"])])
    t = Table(linhas, colWidths=[4.4 * cm, LARGURA_UTIL - 4.4 * cm], repeatRows=1)
    t.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, 0), TINTA),
        ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, FUNDO_SUAVE]),
        ("GRID", (0, 0), (-1, -1), 0.5, LINHA),
        ("LINEBELOW", (0, 0), (-1, 0), 0.5, TINTA),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 7),
        ("RIGHTPADDING", (0, 0), (-1, -1), 7),
        ("TOPPADDING", (0, 0), (-1, -1), 5),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 5),
    ]))
    fluxo.append(t)

    fluxo.append(PageBreak())

    fluxo.append(Paragraph("Nota metodológica", E["h1"]))
    fluxo.append(regua())
    fluxo.append(Paragraph(
        "As cinco categorias pedidas pressupõem uma aplicação web multi-inquilino. O "
        f"{D.PROJETO} é um aplicativo desktop monousuário com API embarcada em loopback, "
        "então cada categoria foi remapeada para o equivalente desta stack antes de ser "
        "aplicada. Onde a categoria não tem correspondente, isso está dito explicitamente "
        "em vez de gerar achado forçado.",
        E["corpo"]))
    fluxo.append(Spacer(1, 3))

    for titulo, texto in D.METODOLOGIA:
        fluxo.append(KeepTogether([
            Paragraph(titulo, E["h3"]),
            Paragraph(texto, E["corpo"]),
        ]))

    linhas = [[Paragraph("Superfície", E["cabecalho_tab"]),
               Paragraph("Volume", E["cabecalho_tab"]),
               Paragraph("Cobertura", E["cabecalho_tab"])]]
    for sup, vol, cob in D.COBERTURA:
        linhas.append([Paragraph(sup, E["celula"]),
                       Paragraph(vol, E["celula"]),
                       Paragraph(f'<font color="#059669"><b>{cob}</b></font>', E["celula"])])
    t = Table(linhas, colWidths=[7.0 * cm, 5.4 * cm, LARGURA_UTIL - 12.4 * cm], repeatRows=1)
    t.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, 0), TINTA),
        ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, FUNDO_SUAVE]),
        ("GRID", (0, 0), (-1, -1), 0.5, LINHA),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 7),
        ("RIGHTPADDING", (0, 0), (-1, -1), 7),
        ("TOPPADDING", (0, 0), (-1, -1), 5),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 5),
    ]))
    fluxo.append(CondPageBreak(11.5 * cm))
    fluxo.append(Paragraph("Cobertura da revisão", E["h2"]))
    fluxo.append(t)
    fluxo.append(Spacer(1, 6))
    fluxo.append(Paragraph(
        "Nenhum achado deste relatório é especulativo: cada um foi verificado no código real, com "
        "arquivo e linha. Onde a exploração depende de uma condição externa — as regras de um "
        "projeto Firebase, a existência de uma segunda conta — a condição está declarada no "
        "próprio achado.",
        E["nota"]))
    return fluxo


def secao_resumo(contagem, por_categoria):
    fluxo = [Spacer(1, 14), CondPageBreak(9 * cm),
             Paragraph("Resumo executivo", E["h1"]), regua()]

    total = sum(contagem.values())
    acionaveis = total - contagem.get("informativa", 0)
    fluxo.append(Paragraph(
        f"A revisão encontrou <b>{total} achados</b>, dos quais <b>{acionaveis}</b> são acionáveis "
        f"e <b>{contagem.get('informativa', 0)}</b> é informativo. "
        f"<b>Nenhum achado crítico.</b> Os <b>{contagem.get('alta', 0)}</b> de severidade alta "
        "concentram-se em dois pontos: a proteção dos dados em repouso (telefone e e-mail que "
        "escapam do envelope criptográfico) e o backup — que reúne, num único arquivo feito para ser "
        "copiado, o banco inteiro, os anexos decifrados e a chave-raiz da instalação. "
        "O restante do sistema resistiu bem: o isolamento por usuário está aplicado em todas as "
        "consultas, nenhum IDOR foi encontrado nos 49 pontos de entrada revisados, o SQL é "
        "integralmente parametrizado e o frontend não tem um único sink de XSS.",
        E["corpo"]))
    fluxo.append(Spacer(1, 4))

    # Tira de placares
    ordem = ["critica", "alta", "media", "baixa", "informativa"]
    celulas, larguras = [], []
    for sev in ordem:
        n = contagem.get(sev, 0)
        celulas.append([
            Paragraph(f'<font color="white" size="19"><b>{n}</b></font>',
                      ParagraphStyle("n", parent=E["celula"], alignment=TA_CENTER, leading=23)),
            Paragraph(f'<font color="white" size="7.8"><b>{D.ROTULO_SEV[sev].upper()}</b></font>',
                      ParagraphStyle("r", parent=E["celula"], alignment=TA_CENTER, leading=11)),
        ])
        larguras.append(LARGURA_UTIL / 5)
    t = Table([celulas], colWidths=larguras, rowHeights=[1.62 * cm])
    estilo = [
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("TOPPADDING", (0, 0), (-1, -1), 7),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 7),
        ("LEFTPADDING", (0, 0), (-1, -1), 3),
        ("RIGHTPADDING", (0, 0), (-1, -1), 3),
    ]
    for i, sev in enumerate(ordem):
        estilo.append(("BACKGROUND", (i, 0), (i, 0), cor(sev)))
        if i:
            estilo.append(("LINEBEFORE", (i, 0), (i, 0), 2.5, colors.white))
    t.setStyle(TableStyle(estilo))
    fluxo.append(t)

    fluxo.append(KeepTogether([
        Paragraph("Distribuição por severidade", E["h2"]),
        grafico_rosca(contagem),
    ]))
    fluxo.append(KeepTogether([
        Paragraph("Achados por categoria auditada", E["h2"]),
        grafico_barras(por_categoria),
    ]))
    return fluxo


def secao_fortes_fracos():
    fluxo = [Spacer(1, 14), CondPageBreak(5.5 * cm),
             Paragraph("Pontos fortes", E["h1"]), regua()]
    fluxo.append(Paragraph(
        "Registrado com evidência de arquivo e linha, tanto para documentar o que está protegido "
        "quanto para demonstrar a cobertura desta auditoria: onde não há achado, é porque o ponto "
        "foi olhado e passou.",
        E["corpo"]))
    fluxo.append(Spacer(1, 3))

    for pf in D.PONTOS_FORTES:
        fluxo.append(KeepTogether([
            caixa(f"✓  {pf['titulo']}", pf["evidencia"], colors.HexColor(D.PALETA["forte"])),
            Spacer(1, 6),
        ]))

    fluxo.append(Spacer(1, 14))
    fluxo.append(CondPageBreak(7 * cm))
    fluxo.append(Paragraph("Pontos fracos", E["h1"]))
    fluxo.append(regua())
    fluxo.append(Paragraph(
        "Os riscos centrais, agrupados por tema em vez de por achado individual.", E["corpo"]))
    fluxo.append(Spacer(1, 3))

    for titulo, texto in D.PONTOS_FRACOS:
        fluxo.append(KeepTogether([
            caixa(f"!  {titulo}", texto, colors.HexColor(D.PALETA["alta"])),
            Spacer(1, 6),
        ]))
    return fluxo


def secao_tabela_achados():
    fluxo = [PageBreak(), Paragraph("Achados detalhados", E["h1"]), regua()]
    fluxo.append(Paragraph(
        "Visão consolidada. Cada achado é desenvolvido logo em seguida, com o trecho de código, a "
        "razão pela qual é explorável, o impacto e as condições de exploração.",
        E["corpo"]))
    fluxo.append(Spacer(1, 4))

    por_cat = {}
    for a in D.ACHADOS:
        por_cat.setdefault(a["categoria"], []).append(a)
    # Ordena pelo número da categoria: agrupar por ordem de aparição dos achados
    # produzia a sequência 1, 3, 4, 2, 5.
    for categoria in sorted(por_cat, key=lambda c: c[:2]):
        itens = por_cat[categoria]
        cabecalho_cat = Paragraph(categoria, E["h2"])
        linhas = [[
            Paragraph("Severidade", E["cabecalho_tab"]),
            Paragraph("Arquivo:linha", E["cabecalho_tab"]),
            Paragraph("Descrição", E["cabecalho_tab"]),
        ]]
        estilo = [
            ("BACKGROUND", (0, 0), (-1, 0), TINTA),
            ("GRID", (0, 0), (-1, -1), 0.5, LINHA),
            ("VALIGN", (0, 0), (-1, -1), "TOP"),
            ("LEFTPADDING", (0, 0), (-1, -1), 6),
            ("RIGHTPADDING", (0, 0), (-1, -1), 6),
            ("TOPPADDING", (0, 0), (-1, -1), 6),
            ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
        ]
        for i, a in enumerate(itens, start=1):
            locais = "<br/>".join(f"{arq}:{ln}" for arq, ln in a["locais"])
            linhas.append([
                chip(D.ROTULO_SEV[a["severidade"]], cor(a["severidade"]), largura=1.85 * cm),
                Paragraph(locais, E["celula_mono"]),
                Paragraph(f'<b>{a["id"]}</b> — {a["titulo"]}', E["celula"]),
            ])
            estilo.append(("BACKGROUND", (0, i), (-1, i),
                           colors.white if i % 2 else FUNDO_SUAVE))
        t = Table(linhas, colWidths=[2.2 * cm, 7.1 * cm, LARGURA_UTIL - 9.3 * cm],
                  repeatRows=1, splitByRow=1)
        t.setStyle(TableStyle(estilo))
        # O título de categoria vai junto com o cabeçalho da tabela, para não
        # ficar sozinho no pé da página.
        fluxo.append(CondPageBreak(4.5 * cm))
        fluxo.append(cabecalho_cat)
        fluxo.append(t)
        fluxo.append(Spacer(1, 8))

    return fluxo


def secao_detalhe_achados():
    fluxo = [PageBreak(), Paragraph("Análise achado a achado", E["h1"]), regua()]

    for a in D.ACHADOS:
        cabecalho = Table(
            [[chip(D.ROTULO_SEV[a["severidade"]], cor(a["severidade"]), largura=1.85 * cm),
              Paragraph(f'<b>{a["id"]}</b>  ·  {a["titulo"]}',
                        ParagraphStyle("ah", parent=E["celula"], fontName=F_BOLD,
                                       fontSize=10.2, leading=13.6))]],
            colWidths=[2.1 * cm, LARGURA_UTIL - 2.1 * cm])
        cabecalho.setStyle(TableStyle([
            ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
            ("LEFTPADDING", (0, 0), (0, 0), 0),
            ("LEFTPADDING", (1, 0), (1, 0), 8),
            ("RIGHTPADDING", (0, 0), (-1, -1), 0),
            ("TOPPADDING", (0, 0), (-1, -1), 0),
            ("BOTTOMPADDING", (0, 0), (-1, -1), 0),
        ]))

        locais = " · ".join(f"<b>{arq}:{ln}</b>" for arq, ln in a["locais"])
        fluxo.append(CondPageBreak(7 * cm))
        fluxo.append(cabecalho)
        fluxo.append(Spacer(1, 4))
        fluxo.append(Paragraph(f'<font color="#475569">{a["categoria"]}</font>', E["nota"]))
        fluxo.append(Paragraph(locais, E["celula_mono"]))
        if a.get("status"):
            fluxo.append(Spacer(1, 5))
            fluxo.append(caixa(
                "SITUACAO ATUAL", a["status"], colors.HexColor(D.PALETA["forte"])
            ))
        fluxo.append(Spacer(1, 6))
        fluxo.append(bloco_codigo(a["trecho"]))
        fluxo.append(Spacer(1, 8))
        fluxo.append(Paragraph("Por que é explorável", E["h3"]))
        fluxo.append(Paragraph(a["porque"], E["corpo"]))
        fluxo.append(caixa("IMPACTO", a["impacto"], cor(a["severidade"])))
        fluxo.append(Spacer(1, 5))
        fluxo.append(caixa("CONDIÇÕES DE EXPLORAÇÃO", a["condicoes"],
                           colors.HexColor("#64748B")))
        fluxo.append(Spacer(1, 5))
        fluxo.append(Paragraph("Correção sugerida", E["h3"]))
        fluxo.append(Paragraph(a["correcao"], E["corpo"]))
        fluxo.append(regua(espaco_antes=6, espaco_depois=12))

    return fluxo


def secao_recomendacoes():
    fluxo = [PageBreak(), Paragraph("Recomendações priorizadas", E["h1"]), regua()]
    fluxo.append(Paragraph(
        "P1 é o que deve entrar na próxima versão: risco alto com correção de baixo custo. "
        "P2 exige migration, decisão de produto ou acesso a um console externo. "
        "P3 é endurecimento — vale fazer, não bloqueia release.",
        E["corpo"]))
    fluxo.append(Spacer(1, 4))

    cores_p = {"P1": D.PALETA["critica"], "P2": D.PALETA["alta"], "P3": D.PALETA["baixa"]}
    linhas = [[
        Paragraph("Prior.", E["cabecalho_tab"]),
        Paragraph("Ação", E["cabecalho_tab"]),
        Paragraph("Detalhe", E["cabecalho_tab"]),
        Paragraph("Achado", E["cabecalho_tab"]),
    ]]
    estilo = [
        ("BACKGROUND", (0, 0), (-1, 0), TINTA),
        ("GRID", (0, 0), (-1, -1), 0.5, LINHA),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 6),
        ("RIGHTPADDING", (0, 0), (-1, -1), 6),
        ("TOPPADDING", (0, 0), (-1, -1), 6),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
        ("ALIGN", (3, 1), (3, -1), "CENTER"),
    ]
    for i, (p, acao, detalhe, ref) in enumerate(D.RECOMENDACOES, start=1):
        linhas.append([
            chip(p, colors.HexColor(cores_p[p]), largura=1.15 * cm),
            Paragraph(f"<b>{acao}</b>", E["celula"]),
            Paragraph(detalhe, E["celula"]),
            Paragraph(f"<b>{ref}</b>", E["celula"]),
        ])
        estilo.append(("BACKGROUND", (0, i), (-1, i),
                       colors.white if i % 2 else FUNDO_SUAVE))
    t = Table(linhas, colWidths=[1.5 * cm, 4.8 * cm, LARGURA_UTIL - 7.9 * cm, 1.6 * cm],
              repeatRows=1)
    t.setStyle(TableStyle(estilo))
    fluxo.append(t)
    return fluxo


def secao_issues():
    fluxo = [PageBreak(), Paragraph("Issues para o GitHub", E["h1"]), regua()]
    fluxo.append(Paragraph(
        "Cada bloco abaixo é o texto completo de uma issue em Markdown, pronto para copiar e colar. "
        "Achados relacionados foram agrupados numa única issue quando compartilham a mesma correção, "
        "para não gerar ruído no backlog.",
        E["corpo"]))
    fluxo.append(Spacer(1, 6))

    acionaveis = [a for a in D.ACHADOS if a["aceite"]]

    for n, a in enumerate(acionaveis, start=1):
        md = montar_markdown_issue(n, a)
        fluxo.append(CondPageBreak(6 * cm))
        fluxo.append(Paragraph(
            f'<font color="#059669"><b>--- ISSUE {n} ---</b></font>',
            ParagraphStyle("issue_marca", parent=E["celula"], fontName=F_MONO,
                           fontSize=8.4, leading=12, spaceAfter=3)))
        fatias = _fatiar(md, E["issue"], por_fatia=4, largura_max=108)
        t = Table(fatias, colWidths=[LARGURA_UTIL], splitByRow=1)
        t.setStyle(TableStyle([
            ("BACKGROUND", (0, 0), (-1, -1), FUNDO_CODIGO),
            ("LINEBEFORE", (0, 0), (0, -1), 2.4, colors.HexColor("#059669")),
            ("LEFTPADDING", (0, 0), (-1, -1), 9),
            ("RIGHTPADDING", (0, 0), (-1, -1), 8),
            ("TOPPADDING", (0, 0), (-1, -1), 0),
            ("BOTTOMPADDING", (0, 0), (-1, -1), 0),
            ("TOPPADDING", (0, 0), (-1, 0), 7),
            ("BOTTOMPADDING", (0, -1), (-1, -1), 7),
            ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ]))
        fluxo.append(t)
        fluxo.append(Paragraph(
            f'<font color="#059669"><b>--- FIM ISSUE {n} ---</b></font>',
            ParagraphStyle("issue_fim", parent=E["celula"], fontName=F_MONO,
                           fontSize=8.4, leading=12, spaceBefore=3, spaceAfter=13)))
    return fluxo


def limpar_html(texto):
    """Remove as tags de ênfase do ReportLab, que não fazem sentido no Markdown."""
    return (texto.replace("<b>", "`").replace("</b>", "`")
            .replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))


def montar_markdown_issue(n, a):
    locais = "\n".join(f"- `{arq}:{ln}`" for arq, ln in a["locais"])
    aceite = "\n".join(f"- [ ] {c}" for c in a["aceite"])
    trecho = a["trecho"].strip("\n")
    situacao = (
        f"\n\n> **Situação:** {limpar_html(a['status'])}" if a.get("status") else ""
    )

    return f"""## {a["issue_titulo"]}

**Labels:** `{a["issue_labels"]}`
**Severidade:** {D.ROTULO_SEV[a["severidade"]]}
**Categoria:** {a["categoria"]}

### Problema

{limpar_html(a["porque"])}

### Evidência

{locais}

```
{trecho}
```

### Impacto

{limpar_html(a["impacto"])}

**Condições de exploração:** {limpar_html(a["condicoes"])}

### Correção sugerida

{limpar_html(a["correcao"])}

### Critérios de aceite

{aceite}

---
_Aberto a partir do relatório de auditoria de segurança de {D.DATA} \
(achado {a["id"]}, versão {D.VERSAO_AUDITADA}, commit {D.COMMIT})._{situacao}"""


# ─── Montagem ────────────────────────────────────────────────────────────────

def main():
    contagem = {}
    for a in D.ACHADOS:
        contagem[a["severidade"]] = contagem.get(a["severidade"], 0) + 1
    for sev in ("critica", "alta", "media", "baixa", "informativa"):
        contagem.setdefault(sev, 0)

    # O mapa categoria -> rotulo curto do grafico vive em achados.py, para que
    # este arquivo nao saiba nada sobre o conteudo da auditoria.
    rotulo_cat = D.ROTULO_CATEGORIA
    ordem_cat = list(dict.fromkeys(rotulo_cat.values()))
    por_categoria = {c: {} for c in ordem_cat}
    for a in D.ACHADOS:
        c = rotulo_cat[a["categoria"]]
        por_categoria[c][a["severidade"]] = por_categoria[c].get(a["severidade"], 0) + 1

    doc = BaseDocTemplate(
        SAIDA, pagesize=A4,
        leftMargin=MARGEM, rightMargin=MARGEM,
        topMargin=MARGEM, bottomMargin=MARGEM,
        title=TITULO, author="Auditoria de segurança",
        subject=f"{D.PROJETO} v{D.VERSAO_AUDITADA}", lang="pt-BR",
    )
    # Um único template: o frame recua 5 mm no topo e na base para não colidir
    # com o cabeçalho e o rodapé desenhados por `decorar`.
    frame = Frame(MARGEM, MARGEM + 5 * mm, LARGURA_UTIL,
                  A4[1] - 2 * MARGEM - 10 * mm,
                  id="corpo", leftPadding=0, rightPadding=0,
                  topPadding=0, bottomPadding=0)
    doc.addPageTemplates([PageTemplate(id="corpo", frames=[frame], onPage=decorar)])

    fluxo = []
    fluxo += secao_capa()
    fluxo += secao_resumo(contagem, por_categoria)
    fluxo += secao_fortes_fracos()
    fluxo += secao_tabela_achados()
    fluxo += secao_detalhe_achados()
    fluxo += secao_recomendacoes()
    fluxo += secao_issues()

    doc.build(fluxo)

    caminho_md = os.path.join(AQUI, "issues-github.md")
    with open(caminho_md, "w", encoding="utf-8") as f:
        f.write(f"# Issues de segurança — {D.PROJETO} v{D.VERSAO_AUDITADA}\n\n")
        f.write(f"Geradas a partir do relatório de auditoria de {D.DATA} "
                f"(commit {D.COMMIT}).\n\n")
        for n, a in enumerate([x for x in D.ACHADOS if x["aceite"]], start=1):
            f.write(f"--- ISSUE {n} ---\n\n")
            f.write(montar_markdown_issue(n, a))
            f.write(f"\n\n--- FIM ISSUE {n} ---\n\n")

    print(f"PDF gerado: {SAIDA}")
    print(f"Issues em Markdown: {caminho_md}")
    print(f"Achados: {sum(contagem.values())} "
          f"({', '.join(f'{D.ROTULO_SEV[s]} {n}' for s, n in contagem.items() if n)})")


if __name__ == "__main__":
    main()
