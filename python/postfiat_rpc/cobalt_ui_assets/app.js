"use strict";

const byId = (id) => document.getElementById(id);
const shortHash = (value) => {
  const text = String(value || "unavailable");
  return text.length <= 28 ? text : `${text.slice(0, 16)}…${text.slice(-8)}`;
};
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
  setRail("trust", trust.ok ? "Verified" : "Failed", trust.ok ? "good" : "hold");
  setText("trust-mode", String(trust.mode).replaceAll("_", " "));
  setText("trust-views", trust.view_count);
  setText("trust-height", trust.activation_height);
  setText("trust-root", shortHash(trust.root));
  setText("trust-source", trust.source);
  const graph = String(trust.active_graph || "—").replace(/^G/, "");
  setText("graph-number", graph);
}

function renderProposals(proposals) {
  const total = (proposals.transition_count || 0) + (proposals.registry_update_count || 0) + (proposals.amendment_count || 0);
  setChip("proposals-status", `${total} recorded`, total ? "good" : "hold");
  setRail("proposals", total ? "Ordered" : "None recorded", total ? "good" : "hold");
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
    item.textContent = "No governance records are present in this node state. The interface does not invent a proposal or expose a creation action.";
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
  setChip("shadow-status", shadow.ok ? "Converged" : "Not converged", shadow.ok ? "good" : "bad");
  setRail("shadow", shadow.ok ? "Converged" : "Failed", shadow.ok ? "good" : "hold");
  setText("shadow-digest", shortHash(shadow.digest));
  setText("shadow-source", shadow.source);
  const grid = byId("node-grid");
  grid.replaceChildren();
  (shadow.nodes || []).forEach((node) => {
    const card = document.createElement("article");
    const good = node.transport_healthy && !node.live_authority && !node.controls_block_consensus;
    card.className = `node-card${good ? " is-good" : ""}`;
    const header = document.createElement("header");
    const name = document.createElement("h3");
    const lamp = document.createElement("span");
    name.textContent = node.node_id;
    lamp.className = "lamp";
    lamp.setAttribute("aria-label", good ? "healthy" : "unhealthy");
    header.append(name, lamp);
    const metrics = document.createElement("dl");
    [["Accepted", node.accepted_messages], ["Queue", node.queue_depth], ["Boots", node.boot_count], ["Peers", node.peer_count]].forEach(([label, value]) => {
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

function renderActivation(activation) {
  const state = activation.ready ? "good" : "hold";
  setChip("activation-status", activation.status, state);
  setRail("activation", activation.status, state);
  setText("gate-decision", activation.status);
  setText("gate-copy", activation.ready
    ? "Every required protocol record is present. Scope remains validator trust evolution only."
    : "Observation may continue, but Cobalt authority must not activate until every gate is evidenced in node state.");
  const readout = document.querySelector(".gate-readout");
  readout.className = `gate-readout${activation.ready ? " is-good" : ""}`;

  const checks = byId("activation-checks");
  checks.replaceChildren();
  (activation.checks || []).forEach((check) => {
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
  setText("witness-count", `${activation.witness_scenarios || 0} scenarios`);
  setText("witness-hash", shortHash(activation.witness_hash));
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
    renderShadow(snapshot.shadow);
    renderActivation(snapshot.activation);
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
