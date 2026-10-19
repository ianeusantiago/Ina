/** A Claude Code permission menu currently on screen ("Do you want to …?" + numbered options). */
export interface PermissionPrompt {
  question: string;
  /** What the agent wants to run or change, e.g. "Write(notes.txt)". */
  action: string;
  /** The block Claude Code shows above the question (command, file, diff), trimmed. */
  detail: string;
  options: { key: string; label: string; kind: "allow" | "always" | "deny" | "other" }[];
}

const QUESTION = /^\s*Do you want to (.+\?)\s*$/;
const OPTION = /^\s*(?:❯\s*)?(\d)\.\s+(.+?)\s*$/;
const TOOL = /^\s*●\s+(.+?)\s*$/;

function kind(label: string): PermissionPrompt["options"][number]["kind"] {
  if (/^Yes, (and|allow|don't ask)/i.test(label)) return "always";
  if (/^Yes\b/i.test(label)) return "allow";
  if (/^No\b/i.test(label)) return "deny";
  return "other";
}

/**
 * Reads the visible terminal lines; returns the active menu or null. Only a menu still waiting
 * for an answer counts (its footer "Esc to cancel" is on screen below the options).
 */
export function parsePermission(lines: string[]): PermissionPrompt | null {
  let q = -1;
  for (let i = lines.length - 1; i >= 0; i--) {
    if (QUESTION.test(lines[i] ?? "")) {
      q = i;
      break;
    }
  }
  if (q < 0) return null;
  const options: PermissionPrompt["options"] = [];
  let footer = false;
  for (const line of lines.slice(q + 1)) {
    const m = OPTION.exec(line);
    if (m?.[1] && m[2]) {
      options.push({ key: m[1], label: m[2], kind: kind(m[2]) });
    } else if (/Esc to cancel/i.test(line)) {
      footer = true;
      break;
    } else if (options.length > 0 && line.trim() && /^\s{4,}/.test(line)) {
      const last = options[options.length - 1];
      if (last) last.label += ` ${line.trim()}`;
    }
  }
  if (!footer || options.length === 0) return null;
  let action = "";
  let start = 0;
  for (let i = q - 1; i >= 0; i--) {
    const m = TOOL.exec(lines[i] ?? "");
    if (m?.[1]) {
      action = m[1];
      start = i + 1;
      break;
    }
  }
  const detail = lines
    .slice(start, q)
    .filter((l) => l.trim() && !/^[\s─╌━-]+$/.test(l))
    .map((l) => l.trimEnd())
    .join("\n")
    .trim();
  return {
    question: `Do you want to ${QUESTION.exec(lines[q] ?? "")?.[1] ?? ""}`,
    action,
    detail,
    options,
  };
}

export function samePrompt(a: PermissionPrompt | null, b: PermissionPrompt | null): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
