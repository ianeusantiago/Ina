import { invoke } from "@tauri-apps/api/core";
import i18n from "../i18n";
import { addHomeItem } from "./homeStore";

export interface Report {
  done: string;
  howToTest: string;
}

const key = (planId: number) => `ina.report.${planId}`;
const inFlight = new Map<number, Promise<Report>>();

function cached(planId: number): Report | null {
  try {
    const raw = localStorage.getItem(key(planId));
    return raw ? (JSON.parse(raw) as Report) : null;
  } catch {
    return null;
  }
}

/** The team's final report, written once by Claude and then kept (one request per team). */
export function teamReport(planId: number): Promise<Report> {
  const hit = cached(planId);
  if (hit) return Promise.resolve(hit);
  let p = inFlight.get(planId);
  if (!p) {
    p = invoke<Report>("team_report", { planId })
      .then((r) => {
        try {
          localStorage.setItem(key(planId), JSON.stringify(r));
        } catch {
          // Not cached; asked again next time.
        }
        return r;
      })
      .finally(() => inFlight.delete(planId));
    inFlight.set(planId, p);
  }
  return p;
}

/** Puts the report in the Home conversation, once per team. */
export async function postReportToHome(planId: number, teamName: string) {
  const posted = `ina.report.home.${planId}`;
  try {
    if (localStorage.getItem(posted)) return;
    const r = await teamReport(planId);
    localStorage.setItem(posted, "1");
    addHomeItem({
      kind: "agent",
      text: `${i18n.t("report.homeTitle", { name: teamName })}\n\n${r.done}\n\n${i18n.t("report.howToTest")}\n${r.howToTest}`,
    });
  } catch (e) {
    console.error("[report]", e);
  }
}
