"use strict";

const $ = (id) => document.getElementById(id);
const VERDICT_CLASS = { ok: "verdict--ok", pending: "verdict--wait" };

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

// Renders one server-built section: its verdict, the reasons when it is not
// ok (a refusal is shown as a refusal), and every field as a row.
function renderSection(section) {
  const panel = element("section", "panel");
  panel.append(element("h2", "", section.title));
  const verdict = element("p", `verdict ${VERDICT_CLASS[section.status] || "verdict--bad"}`, section.verdict);
  verdict.dataset.status = section.status;
  panel.append(verdict);
  if (section.reasons.length > 0) {
    const reasons = element("ul", "reasons");
    for (const reason of section.reasons) reasons.append(element("li", "", reason));
    panel.append(reasons);
  }
  if (section.rows.length > 0) {
    const facts = element("dl", "facts");
    for (const [label, value] of section.rows) {
      const row = element("div");
      row.append(element("dt", "", label), element("dd", "", value));
      facts.append(row);
    }
    panel.append(facts);
  }
  return panel;
}

function render(state) {
  $("endpoint").textContent = state.endpoint_label || "validator";
  $("pool").textContent = state.pool_id;
  $("next-height").textContent = state.next_height === null ? "—" : String(state.next_height);
  $("sections").replaceChildren(...state.view.sections.map(renderSection));
}

async function refresh() {
  const button = $("refresh");
  const status = $("status");
  button.disabled = true;
  status.className = "status";
  status.textContent = "Reading the validator…";
  try {
    const response = await fetch("/api/state", { cache: "no-store" });
    const body = await response.json();
    if (!response.ok) throw new Error(`Failed: ${body.error}`);
    render(body);
    status.textContent = `Fetched ${body.fetched_at}`;
  } catch (error) {
    status.className = "status status--error";
    status.textContent = error.message;
  } finally {
    button.disabled = false;
  }
}

async function readReceipt() {
  const intentId = $("intent-id").value.trim();
  const target = $("receipt");
  try {
    const response = await fetch(`/api/receipt?intent_id=${encodeURIComponent(intentId)}`, { cache: "no-store" });
    const body = await response.json();
    if (!response.ok) {
      target.replaceChildren(element("p", "verdict verdict--bad", `${body.refused ? "Refused" : "Failed"}: ${body.error}`));
      return;
    }
    target.replaceChildren(renderSection(body.view));
  } catch (error) {
    target.replaceChildren(element("p", "verdict verdict--bad", `Failed: ${error.message}`));
  }
}

document.addEventListener("DOMContentLoaded", () => {
  $("refresh").addEventListener("click", refresh);
  $("receipt-read").addEventListener("click", readReceipt);
  refresh();
});
