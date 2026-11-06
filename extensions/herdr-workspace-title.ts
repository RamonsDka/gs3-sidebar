// @ts-nocheck
import { execFile } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";

/**
 * Herdr Workspace Title Extension for Pi / Gentle-Shell
 *
 * Renombra el espacio (workspace) de Herdr una sola vez, con un título corto
 * derivado del primer pedido real del usuario en esa sesión.
 *
 * Reglas:
 * - Solo actúa dentro de un pane de Herdr (HERDR_ENV=1 + HERDR_WORKSPACE_ID).
 * - Solo titula si el espacio todavía no tiene nombre propio (`custom_name` en
 *   session.json). Un rename manual del usuario manda y no se pisa nunca.
 * - El título lo redacta el modelo activo con una llamada corta y aislada; si
 *   falla (sin auth, timeout, respuesta vacía) cae a un heurístico local.
 * - Corre en segundo plano: nunca bloquea ni retrasa el turno del agente.
 * - Rechequea inmediatamente antes del rename asíncrono para evitar condiciones de carrera.
 * - Un rename fallido permite reintento en el siguiente prompt elegible.
 * - Evita renames duplicados ante hooks concurrentes.
 */

const MAX_TITLE_CHARS = 34;
const MIN_PROMPT_CHARS = 12;
const MODEL_TIMEOUT_MS = 15000;

const TITLE_SYSTEM_PROMPT = [
	"You name terminal workspaces for a developer.",
	"The user gives you the first request of a work session.",
	"Reply with ONLY a short title for that session: 2 to 5 words, at most 32 characters.",
	"Write the title in the same language as the request, describing the task, not the tool chatter.",
	"Keep the language's own diacritics: in Spanish write 'Título', never 'Titulo'.",
	"No quotes, no emojis, no trailing period, no prefixes like 'Title:' or 'Workspace:'.",
	"Examples:",
	"request: 'necesito que en herdr el titulo del espacio se ponga solo' -> Editar titulos de herdr",
	"request: 'fix the login redirect loop in the auth middleware' -> Fix login redirect loop",
].join("\n");

const STOPWORDS = new Set([
	// es
	"para", "porque", "cuando", "donde", "como", "quiero", "necesito", "hacer", "hacerlo", "tengo",
	"sobre", "desde", "hasta", "entre", "pero", "este", "esta", "esto", "estos", "estas", "esos",
	"esas", "aquel", "aquella", "tambien", "solo", "sola", "cosas", "cosa", "algo", "algun", "alguna",
	"que", "con", "sin", "los", "las", "una", "uno", "unos", "unas", "del", "mas", "muy", "hay",
	"seria", "pueda", "puedas", "puedo", "deberia", "quisiera", "ademas", "entonces", "ahora",
	"favor", "ayuda", "podrias", "gustaria", "este", "seccion", "espacio",
	// en
	"the", "and", "for", "with", "that", "this", "these", "those", "from", "into", "want", "need",
	"please", "should", "would", "could", "have", "has", "you", "your", "our", "its", "add", "make",
]);

function getWorkspaceId(): string | undefined {
	return process.env.HERDR_WORKSPACE_ID?.trim();
}

function getHerdrBin(): string {
	return process.env.HERDR_BIN_PATH ?? "herdr";
}

function getHerdrSessionFile(): string {
	return (
		process.env.HERDR_SESSION_FILE ??
		(process.platform === "win32"
			? path.join(process.env.APPDATA ?? path.join(os.homedir(), "AppData", "Roaming"), "herdr", "session.json")
			: path.join(os.homedir(), ".config", "herdr", "session.json"))
	);
}

function log(message: string): void {
	if (process.env.HERDR_TITLE_DEBUG !== "1") return;
	try {
		fs.appendFileSync(path.join(os.tmpdir(), "herdr-workspace-title.log"), `${new Date().toISOString()} ${message}\n`);
	} catch {}
}

export function sanitizeTitle(raw: string | null | undefined): string | null {
	if (typeof raw !== "string") return null;
	const line = raw
		.split(/\r?\n/)
		.map((entry) => entry.trim())
		.find((entry) => entry.length > 0);
	if (!line) return null;

	const title = line
		.replace(/^[#>*\-\s]+/, "")
		.replace(/^(?:t[ií]tulo|title|nombre|name|workspace|espacio|secci[oó]n)\s*[:=\-–]\s*/i, "")
		.replace(/^[\p{Extended_Pictographic}\p{Emoji_Presentation}\s]+/u, "")
		.replace(/["'`“”‘’«»]/g, "")
		.replace(/\s+/g, " ")
		.replace(/[.,;:!?¿¡、。]+$/u, "")
		.trim();
	if (!title) return null;

	if (title.length <= MAX_TITLE_CHARS) return title;

	const clipped = title.slice(0, MAX_TITLE_CHARS + 1);
	const lastSpace = clipped.lastIndexOf(" ");
	return (lastSpace > 10 ? clipped.slice(0, lastSpace) : title.slice(0, MAX_TITLE_CHARS)).trim();
}

export function heuristicTitle(prompt: string | null | undefined): string | null {
	if (typeof prompt !== "string") return null;
	const text = prompt
		.replace(/```[\s\S]*?```/g, " ")
		.replace(/`[^`]*`/g, " ")
		.replace(/\s+/g, " ")
		.trim();
	if (!text) return null;

	const words = text
		.toLowerCase()
		.replace(/[^\p{L}\p{N}\s]/gu, " ")
		.split(/\s+/)
		.filter((word) => word.length > 2 && !STOPWORDS.has(word));
	if (words.length === 0) return null;

	return sanitizeTitle(words.slice(0, 4).join(" "));
}

function readWorkspaceEntry(): { custom_name?: string | null; identity_cwd?: string | null } | null {
	const wsId = getWorkspaceId();
	if (!wsId) return null;
	try {
		const raw = fs.readFileSync(getHerdrSessionFile(), "utf-8");
		const data = JSON.parse(raw);
		if (!Array.isArray(data?.workspaces)) return null;
		const workspace = data.workspaces.find((entry: any) => entry?.id === wsId);
		if (!workspace || typeof workspace !== "object") return null;
		return workspace;
	} catch {
		return null;
	}
}

export function readCustomName(): string | null {
	const entry = readWorkspaceEntry();
	const name = entry?.custom_name;
	return typeof name === "string" && name.trim().length > 0 ? name.trim() : null;
}

function readIdentityCwd(): string | null {
	const entry = readWorkspaceEntry();
	const cwd = entry?.identity_cwd;
	return typeof cwd === "string" && cwd.trim().length > 0 ? cwd.trim() : null;
}

/** Etiqueta que Herdr derivaría solo del cwd del espacio ("~" para $HOME). */
export function autoLabelFor(identityCwd: string | null | undefined): string | null {
	if (typeof identityCwd !== "string" || identityCwd.trim().length === 0) return null;
	const cleaned = identityCwd.replace(/[\\/]+$/, "");
	if (cleaned === os.homedir().replace(/[\\/]+$/, "")) return "~";
	const base = path.basename(cleaned);
	return base.length > 0 ? base : null;
}

function fetchLiveWorkspace(): Promise<{ label: string; identityCwd: string } | null> {
	const wsId = getWorkspaceId();
	if (!wsId) return Promise.resolve(null);
	const identityCwd = readIdentityCwd();
	if (!identityCwd) return Promise.resolve(null);

	return new Promise((resolve) => {
		execFile(getHerdrBin(), ["workspace", "get", wsId], { timeout: 2000, windowsHide: true }, (error, stdout) => {
			if (error) {
				resolve(null);
				return;
			}
			try {
				const workspace = JSON.parse(stdout)?.result?.workspace;
				const label = typeof workspace?.label === "string" && workspace.label.trim().length > 0 ? workspace.label : null;
				if (!label) {
					resolve(null);
					return;
				}
				resolve({ label, identityCwd });
			} catch {
				resolve(null);
			}
		});
	});
}

/**
 * Decide si el espacio sigue libre para titulado automático.
 *
 * Es conservador a propósito: ante la duda NO toca el título (fail closed),
 * porque pisar un nombre puesto a mano es mucho peor que dejar un espacio sin titular.
 * - Falla cerrado (false) si la sesión no puede leerse, si el cwd es desconocido,
 *   si la consulta a Herdr falla o devuelve JSON inválido/vacío.
 * - Falla cerrado (false) si `custom_name` existe en session.json.
 * - Falla cerrado (false) si la etiqueta viva difiere de la derivada automáticamente del cwd.
 */
export async function shouldAutoTitle(): Promise<boolean> {
	const entry = readWorkspaceEntry();
	if (!entry) {
		log(`skip: session file unreadable or workspace ${getWorkspaceId()} not found in session`);
		return false;
	}
	if (typeof entry.custom_name === "string" && entry.custom_name.trim().length > 0) {
		return false;
	}

	const live = await fetchLiveWorkspace();
	if (!live) {
		log(`skip: live workspace query failed, malformed, or missing cwd for ${getWorkspaceId()}`);
		return false;
	}

	const autoLabel = autoLabelFor(live.identityCwd);
	if (!autoLabel) {
		log(`skip: cannot determine auto-label for cwd "${live.identityCwd}"`);
		return false;
	}

	if (live.label !== autoLabel) {
		log(`skip: ${getWorkspaceId()} ya tiene etiqueta propia "${live.label}"`);
		return false;
	}

	return true;
}

async function generateTitle(prompt: string, ctx: ExtensionContext): Promise<string | null> {
	const model = ctx?.model;
	const registry: any = ctx?.modelRegistry;
	if (!model || typeof registry?.complete !== "function") return null;

	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), MODEL_TIMEOUT_MS);
	try {
		const reply = await registry.complete(
			model,
			{
				systemPrompt: TITLE_SYSTEM_PROMPT,
				messages: [{ role: "user", content: prompt.slice(0, 2000), timestamp: Date.now() }],
			},
			{ maxTokens: 40, temperature: 0, signal: controller.signal },
		);
		const text = (reply?.content ?? [])
			.filter((part: any) => part?.type === "text")
			.map((part: any) => part?.text ?? "")
			.join(" ");
		return sanitizeTitle(text);
	} catch (error) {
		log(`nested model call failed: ${error instanceof Error ? error.message : String(error)}`);
		return null;
	} finally {
		clearTimeout(timer);
	}
}

function renameWorkspace(title: string, source: string): Promise<boolean> {
	const wsId = getWorkspaceId();
	if (!wsId) return Promise.resolve(false);
	return new Promise((resolve) => {
		execFile(
			getHerdrBin(),
			["workspace", "rename", wsId, title],
			{ timeout: 3000, windowsHide: true },
			(error, stdout, stderr) => {
				if (error) {
					log(`rename failed: ${error.message} ${stdout ?? ""} ${stderr ?? ""}`);
					resolve(false);
					return;
				}
				log(`renamed ${wsId} -> "${title}" (source=${source})`);
				resolve(true);
			},
		);
	});
}

function enabled(): boolean {
	return (
		process.env.HERDR_TITLE_DISABLE !== "1" &&
		process.env.HERDR_ENV === "1" &&
		typeof getWorkspaceId() === "string" &&
		getWorkspaceId()!.length > 0
	);
}

let lastTask: Promise<void> | null = null;

export function getLastTask(): Promise<void> | null {
	return lastTask;
}

export default function initHerdrWorkspaceTitle(pi: ExtensionAPI): void {
	if (!enabled()) return;

	let handled = false;
	let inFlight = false;

	pi.on("before_agent_start", (event: any, ctx: ExtensionContext) => {
		if (event?.messageType !== "user") return;
		if (handled || inFlight) return;

		const prompt = typeof event.prompt === "string" ? event.prompt.trim() : "";
		if (prompt.length < MIN_PROMPT_CHARS) return;

		handled = true;
		inFlight = true;
		const task = (async () => {
			try {
				if (!(await shouldAutoTitle())) return;

				let source = "model";
				let title = await generateTitle(prompt, ctx);
				if (!title) {
					source = "heuristic";
					title = heuristicTitle(prompt);
				}
				if (!title) {
					// Sin título no quemamos la oportunidad: el próximo pedido reintenta.
					handled = false;
					return;
				}

				// Rechequeo inmediato antes de renombrar asíncronamente
				if (!(await shouldAutoTitle())) return;

				const ok = await renameWorkspace(title, source);
				if (!ok) {
					// Fallo en rename permite reintento en el próximo prompt elegible
					handled = false;
				}
			} finally {
				inFlight = false;
			}
		})();
		lastTask = task;
		void task;
	});
}
