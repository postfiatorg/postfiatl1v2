"use strict";

const byId = (id) => document.getElementById(id);
const shortHash = (value) => {
  const text = String(value || "unavailable");
  return text.length <= 28 ? text : `${text.slice(0, 16)}…${text.slice(-8)}`;
};
const yesNo = (value) => value ? "Yes" : "No";
const setText = (id, value) => { byId(id).textContent = value ?? "—"; };
const setChip = (id, label, state) => {
  const node = byId(id);
  node.textContent = label;
  node.className = `state-chip is-${state}`;
};
const setRail = (name, label, state) => {
  const node = document.querySelector(`[data-rail="${name}"]`);
  node.className = `rail-node is-${state}`;
  node.querySelector("em").textContent = label;
};

function renderTrust(trust) {
  setChip("trust-status", trust.ok ? "Verified" : "Unavailable", trust.ok ? "good" : "bad");
  setText("trust-mode", String(trust.mode).replaceAll("_", " "));
  setText("trust-views", trust.view_count);
  setText("trust-height", trust.activation_height);
  setText("trust-root", shortHash(trust.root));
  setText("trust-source", trust.source);
  setText("graph-number", String(trust.active_graph || "—").replace(/^G/, ""));
}

function renderProposals(proposals) {
  const total = (proposals.transition_count || 0) + (proposals.registry_update_count || 0) + (proposals.amendment_count || 0);
  setChip("proposals-status", `${total} recorded`, total ? "good" : "fact");
  setText("authority-label", proposals.authority_label);
  setText("transition-count", proposals.transition_count || 0);
  setText("registry-count", proposals.registry_update_count || 0);
  setText("amendment-count", proposals.amendment_count || 0);
  setText("proposal-source", proposals.source);

  const list = byId("proposal-list");
  list.replaceChildren();
  if (!proposals.items || proposals.items.length === 0) {
    const item = document.createElement("li");
    item.className = "empty";
    item.textContent = "No governance records are present. This interface does not invent proposals or expose creation actions.";
    list.append(item);
    return;
  }
  proposals.items.slice().reverse().forEach((proposal) => {
    const item = document.createElement("li");
    const title = document.createElement("strong");
    const detail = document.createElement("span");
    const status = document.createElement("em");
    title.textContent = `${proposal.type} / ${proposal.id}`;
    detail.textContent = `${proposal.detail} · height ${proposal.height ?? "—"}`;
    status.textContent = proposal.status;
    item.append(title, status, detail);
    list.append(item);
  });
}

function renderShadow(shadow) {
  const state = shadow.ok ? "good" : "bad";
  setChip("shadow-status", shadow.ok ? "Healthy" : "Not healthy", state);
  setRail("shadow", shadow.ok ? "Healthy" : "Failed", state);
  setText("shadow-digest", shortHash(shadow.digest));
  setText("shadow-source", shadow.source);
  const grid = byId("node-grid");
  grid.replaceChildren();
  (shadow.nodes || []).forEach((node) => {
    const card = document.createElement("article");
    const good = node.transport_healthy && node.catch_up_status === "current" && node.contiguous_sequence === node.protocol_decision_count && !node.live_authority && !node.controls_block_consensus;
    card.className = `node-card${good ? " is-good" : ""}`;
    const header = document.createElement("header");
    const name = document.createElement("h3");
    const lamp = document.createElement("span");
    name.textContent = node.node_id;
    lamp.className = "lamp";
    lamp.setAttribute("aria-label", good ? "healthy" : "unhealthy");
    header.append(name, lamp);
    const metrics = document.createElement("dl");
    [["Accepted", node.accepted_messages], ["History", node.contiguous_sequence], ["Cert signers", node.certificate_signer_count], ["Catch-up", node.catch_up_status], ["Boots", node.boot_count], ["Peers", node.peer_count]].forEach(([label, value]) => {
      const cell = document.createElement("div");
      const term = document.createElement("dt");
      const description = document.createElement("dd");
      term.textContent = label;
      description.textContent = value;
      cell.append(term, description);
      metrics.append(cell);
    });
    card.append(header, metrics);
    grid.append(card);
  });
  if (!shadow.nodes || shadow.nodes.length === 0) {
    const empty = document.createElement("p");
    empty.className = "provenance";
    empty.textContent = "No persisted shadow node status was available.";
    grid.append(empty);
  }
}

function renderReadiness(readiness, scenario) {
  const state = readiness.ready ? "good" : "hold";
  setChip("readiness-status", readiness.status, state);
  setRail("readiness", readiness.status, state);
  setText("readiness-decision", readiness.status);
  setText("readiness-copy", readiness.ready
    ? "Evidence supports a separately authorized controlled-testnet validator-trust cutover. No activation has occurred."
    : "Evidence is incomplete or failed. Keep Foundation authority and remediate the failed checks.");
  const readout = byId("readiness-readout");
  readout.className = `gate-readout${readiness.ready ? " is-good" : ""}`;

  setText("scenario-cases", scenario.case_count ?? "—");
  setText("scenario-passes", scenario.case_count == null ? "—" : `${scenario.cobalt_passed}/${scenario.rippled_passed}`);
  setText("scenario-conflicts", scenario.cobalt_conflicting_decisions == null ? "—" : `${scenario.cobalt_conflicting_decisions}/${scenario.rippled_conflicting_decisions}`);

  const checks = byId("readiness-checks");
  checks.replaceChildren();
  (readiness.checks || []).forEach((check) => {
    const item = document.createElement("li");
    if (check.ok) item.className = "is-good";
    const mark = document.createElement("b");
    const label = document.createElement("strong");
    const source = document.createElement("span");
    mark.textContent = check.ok ? "✓" : "×";
    label.textContent = check.label;
    source.textContent = check.source;
    item.append(mark, label, source);
    checks.append(item);
  });
  setText("benchmark-root", shortHash(readiness.packets?.benchmark?.manifest_sha256));
  setText("handoff-root", shortHash(readiness.packets?.handoff?.manifest_sha256));
}

function renderAuthority(authority) {
  const label = authority.known ? authority.label : "Unavailable";
  setChip("actual-authority-status", label, authority.known ? "fact" : "bad");
  setRail("authority", label, authority.known ? "fact" : "bad");
  setText("actual-authority-label", label);
  setText("foundation-active", authority.known ? yesNo(authority.foundation_active) : "Unknown");
  setText("cobalt-active", authority.known ? yesNo(authority.cobalt_active) : "Unknown");
  setText("block-finality", authority.block_finality || "Unknown");
  setText("cobalt-block-control", yesNo(authority.controls_block_consensus));
  setText("actual-transition-count", authority.transition_count);
  setText("actual-authority-source", authority.source);
}

function renderErrors(errors) {
  const consoleNode = byId("error-console");
  const list = byId("error-list");
  list.replaceChildren();
  consoleNode.hidden = !errors || errors.length === 0;
  (errors || []).forEach((message) => {
    const item = document.createElement("li");
    item.textContent = message;
    list.append(item);
  });
}

async function refresh(force = false) {
  const button = byId("refresh");
  button.disabled = true;
  button.textContent = "Scanning…";
  try {
    const response = await fetch(`/api/snapshot${force ? "?refresh=1" : ""}`, { cache: "no-store" });
    if (!response.ok) throw new Error(`snapshot request failed: ${response.status}`);
    const snapshot = await response.json();
    setText("collected-at", new Date(snapshot.collected_at).toLocaleString());
    renderTrust(snapshot.trust);
    renderProposals(snapshot.proposals);
    renderShadow(snapshot.shadow_health);
    renderReadiness(snapshot.rehearsal_readiness, snapshot.scenario);
    renderAuthority(snapshot.actual_authority);
    renderErrors(snapshot.errors);
  } catch (error) {
    renderErrors([String(error)]);
  } finally {
    button.disabled = false;
    button.textContent = "Refresh surfaces";
  }
}

byId("refresh").addEventListener("click", () => refresh(true));
refresh(false);
window.setInterval(() => refresh(false), 15000);
