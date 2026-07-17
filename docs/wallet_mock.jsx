import React, { useState } from "react";

/* ==================================================================
 * PostFiat Wallet — composition rebuild
 *
 * Prior pass fixed the chrome but left every screen top-anchored in an
 * empty frame. This pass makes each screen INHABIT its frame:
 *  · Dashboard + NavCoins fill the canvas top-to-bottom (flex height,
 *    a summary stat row, a designed lower edge).
 *  · Send / Swap / More become balanced, centered compositions —
 *    intentional negative space, not a form jammed in the corner.
 *  · NavCoins sorts by the signal (widest discount on top) and the
 *    NAV axis is a real, continuous, labeled line through the bars.
 *  · Home holdings show value at MARKET (realizable) so the figure no
 *    longer contradicts its bar. NAV shown as muted context.
 *  · Activity gets a full-width column; rows never wrap or clip.
 *  · The false-active Swap highlight on home is gone.
 * ================================================================== */

const STYLE = `
:root{
  --bg:#0a0a0a; --sidebar:#0c0c0c; --surface:#141414; --surface2:#181818;
  --raised:#1f1f1f; --border:#262626; --border-soft:#1b1b1b; --axis:#57575f;
  --text:#f4f4f5; --muted:#a1a1aa; --dim:#6f6f78;
  --green:#85e07b; --green-ink:#08120a; --mint:#b9f3ae;
  --green-soft:rgba(133,224,123,0.12); --green-border:rgba(133,224,123,0.40);
  --amber:#e0a45c; --red:#ef6a6a; --red-soft:rgba(239,106,106,0.10);
  --mono:ui-monospace,"SF Mono",SFMono-Regular,Menlo,Consolas,monospace;
  --sans:-apple-system,BlinkMacSystemFont,"Inter","Segoe UI",Roboto,system-ui,sans-serif;
}
*{box-sizing:border-box}
.pf-root{height:100vh;background:var(--bg);color:var(--text);font-family:var(--sans);}
button{font-family:inherit}
input,select{font-family:inherit}

/* ---- shell / frame-filling main ---- */
.pf-shell{display:grid;grid-template-columns:248px minmax(0,1fr);height:100vh;}
.pf-sidebar{display:flex;flex-direction:column;gap:4px;padding:20px 14px;
  border-right:1px solid var(--border-soft);height:100vh;background:var(--sidebar);}
.pf-main{min-width:0;height:100vh;display:flex;flex-direction:column;}
.pf-page{flex:1;min-height:0;display:flex;flex-direction:column;width:100%;max-width:1180px;
  margin:0 auto;padding:30px 44px;overflow:auto;}
.pf-stage-inner{margin:auto;width:100%;}
.pf-topbar{display:none;}
.pf-bottomnav{display:none;}

@media(max-width:920px){
  .pf-root{height:auto;}
  .pf-shell{grid-template-columns:1fr;height:auto;}
  .pf-sidebar{display:none;}
  .pf-main{height:auto;display:block;}
  .pf-page{display:block;max-width:none;padding:18px 16px 90px;overflow:visible;}
  .pf-stage-inner{margin:0;}
  .pf-topbar{display:flex;align-items:center;justify-content:space-between;
    padding:14px 16px;border-bottom:1px solid var(--border-soft);position:sticky;top:0;z-index:20;
    background:rgba(10,10,10,0.9);backdrop-filter:blur(12px);}
  .pf-bottomnav{display:grid;grid-template-columns:repeat(5,1fr);gap:2px;position:fixed;bottom:0;left:0;right:0;z-index:20;
    padding:9px 8px calc(9px + env(safe-area-inset-bottom));
    background:rgba(10,10,10,0.92);backdrop-filter:blur(12px);border-top:1px solid var(--border-soft);}
}

/* ---- sidebar ---- */
.pf-brand{display:flex;align-items:center;gap:11px;padding:6px 8px 14px;}
.pf-mark{width:36px;height:36px;border-radius:10px;background:var(--green);color:var(--green-ink);
  display:grid;place-items:center;font-family:var(--mono);font-weight:700;font-size:13px;}
.pf-chip{display:inline-flex;align-items:center;gap:8px;background:var(--surface2);
  border:1px solid var(--border);border-radius:999px;padding:6px 12px;font-family:var(--mono);
  font-size:12px;color:var(--muted);cursor:pointer;}
.pf-dot{width:6px;height:6px;border-radius:999px;background:var(--green);}
.pf-nav{display:flex;align-items:center;gap:12px;padding:11px 12px;border-radius:10px;color:var(--muted);
  cursor:pointer;font-size:14px;font-weight:550;border:1px solid transparent;text-align:left;background:none;}
.pf-nav:hover{background:var(--surface);color:var(--text);}
.pf-nav.on{background:var(--green-soft);border-color:var(--green-border);color:var(--mint);}
.pf-nav-badge{width:22px;height:22px;border-radius:6px;display:grid;place-items:center;
  font-family:var(--mono);font-size:11px;border:1px solid currentColor;}
.pf-ledger{margin-top:auto;font-family:var(--mono);font-size:10.5px;color:var(--dim);line-height:1.6;
  padding:10px 10px 4px;border-top:1px solid var(--border-soft);}

.pf-bnav{background:none;border:none;cursor:pointer;display:flex;flex-direction:column;align-items:center;gap:5px;padding:6px 0 4px;}
.pf-bnav-badge{width:24px;height:24px;border-radius:999px;display:grid;place-items:center;
  font-family:var(--mono);font-size:11px;border:1px solid var(--border);color:var(--dim);}
.pf-bnav.on .pf-bnav-badge{border-color:var(--green-border);background:var(--green-soft);color:var(--mint);}
.pf-bnav-l{font-size:10.5px;color:var(--dim);}
.pf-bnav.on .pf-bnav-l{color:var(--text);}

/* ---- type ---- */
.pf-eyebrow{font-family:var(--mono);font-size:11px;letter-spacing:0.14em;text-transform:uppercase;color:var(--dim);}
.pf-h1{font-size:30px;font-weight:700;letter-spacing:-0.03em;margin:6px 0 0;}
@media(max-width:560px){.pf-h1{font-size:25px;}}

/* ---- cards / controls ---- */
.pf-card{background:var(--surface);border:1px solid var(--border);border-radius:16px;padding:18px;}
.pf-primary{width:100%;background:var(--green);color:var(--green-ink);border:none;border-radius:12px;
  padding:14px 18px;font-size:15px;font-weight:650;letter-spacing:-0.01em;cursor:pointer;transition:filter .15s;}
.pf-primary:hover{filter:brightness(1.06);}
.pf-primary:disabled{background:var(--raised);color:var(--dim);cursor:default;filter:none;}
.pf-ghost{background:transparent;color:var(--text);border:1px solid var(--border);border-radius:12px;
  padding:13px 16px;font-size:14px;font-weight:600;cursor:pointer;transition:all .15s;width:100%;}
.pf-ghost:hover{border-color:#333;}
.pf-ghost.on{background:var(--green-soft);border-color:var(--green-border);color:var(--mint);}
.pf-input,.pf-select{width:100%;background:var(--surface);border:1px solid var(--border);border-radius:12px;
  padding:13px 15px;color:var(--text);font-size:14px;outline:none;}
.pf-input{font-family:var(--mono);font-size:13.5px;}
.pf-select{cursor:pointer;}
:where(button,a,input,select,[role=button]):focus-visible{outline:2px solid var(--green-border);outline-offset:2px;}

.pf-pill{font-family:var(--mono);font-size:11px;letter-spacing:0.04em;padding:4px 9px;border-radius:999px;
  background:var(--surface2);border:1px solid var(--border);color:var(--muted);white-space:nowrap;}
.pf-pill.good{background:var(--green-soft);border-color:var(--green-border);color:var(--mint);}

.pf-row{display:flex;justify-content:space-between;align-items:center;}
.pf-rk{font-size:13px;color:var(--muted);}
.pf-rv{font-family:var(--mono);font-size:12.5px;color:var(--text);}
.pf-link{background:none;border:none;color:var(--mint);font-family:var(--mono);font-size:11px;cursor:pointer;padding:0;letter-spacing:0.04em;}

/* ---- dashboard ---- */
.pf-band{display:flex;justify-content:space-between;align-items:flex-end;gap:24px;margin-bottom:22px;flex-wrap:wrap;}
.pf-actions{display:flex;gap:10px;}
.pf-actions .pf-ghost{width:auto;padding:13px 22px;}
.pf-stats{display:grid;grid-template-columns:repeat(3,1fr);gap:14px;margin-bottom:20px;}
.pf-tile{background:var(--surface);border:1px solid var(--border);border-radius:14px;padding:15px 18px;
  display:flex;flex-direction:column;gap:5px;text-align:left;}
button.pf-tile{cursor:pointer;transition:border-color .15s;}
button.pf-tile:hover{border-color:#333;}
.pf-dash{flex:1;min-height:0;display:grid;grid-template-columns:1.5fr 1fr;gap:20px;}
.pf-dash-col{display:flex;flex-direction:column;gap:18px;min-height:0;}
.pf-activity-card{display:flex;flex-direction:column;min-height:0;padding:6px 6px;}
.pf-feed{flex:1;min-height:0;overflow:auto;}
.pf-act{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:13px 14px;border-bottom:1px solid var(--border-soft);}
.pf-act:last-child{border-bottom:none;}
.pf-act-l{min-width:0;}
.pf-act-t{font-size:14px;font-weight:550;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;}
.pf-act-s{font-family:var(--mono);font-size:11px;color:var(--dim);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;margin-top:2px;}
.pf-act-v{font-family:var(--mono);font-size:12.5px;white-space:nowrap;}
@media(max-width:920px){
  .pf-stats{grid-template-columns:1fr;}
  .pf-dash{grid-template-columns:1fr;}
  .pf-feed{max-height:none;}
}

/* ---- compositions (send/swap/more) ---- */
.pf-two{display:grid;grid-template-columns:1.35fr 1fr;gap:20px;align-items:start;}
.pf-even{display:grid;grid-template-columns:1fr 1fr;gap:16px;}
@media(max-width:920px){.pf-two,.pf-even{grid-template-columns:1fr;}}

/* ---- premium bar (home/detail, with inline label) ---- */
.pf-prem{display:flex;align-items:center;gap:12px;}
.pf-prem-track{position:relative;flex:1;background:var(--raised);border-radius:999px;}
.pf-prem-axis{position:absolute;left:50%;top:-7px;bottom:-7px;width:2px;background:var(--axis);transform:translateX(-50%);}
.pf-prem-bar{position:absolute;top:0;border-radius:999px;}
.pf-prem-label{font-family:var(--mono);font-size:12px;min-width:52px;text-align:right;}

/* ---- navcoins comparison table ---- */
.pf-thead,.pf-trow-d{display:grid;grid-template-columns:1.7fr 0.7fr 1fr 1fr 2fr 0.7fr 24px;gap:16px;align-items:center;}
.pf-thead{padding:2px 16px 12px;border-bottom:1px solid var(--border-soft);}
.pf-th{font-family:var(--mono);font-size:10px;letter-spacing:0.12em;text-transform:uppercase;color:var(--dim);
  background:none;border:none;padding:0;cursor:pointer;display:inline-flex;align-items:center;gap:5px;}
.pf-th.r{justify-content:flex-end;}
.pf-th.c{justify-content:center;}
.pf-th:hover{color:var(--muted);}
.pf-th.active{color:var(--mint);}
.pf-trow-d{padding:18px 16px;border-bottom:1px solid var(--border-soft);cursor:pointer;transition:background .12s;}
.pf-trow-d:hover{background:var(--surface2);}
.pf-trow-d:last-child{border-bottom:none;}
.pf-trow-m{display:none;}
.pf-num{font-family:var(--mono);font-size:13.5px;text-align:right;}

/* continuous, full-height NAV axis through the bar column */
.pf-navbar{position:relative;align-self:stretch;display:flex;align-items:center;margin:-18px 0;}
.pf-navbar-axis{position:absolute;left:50%;top:0;bottom:0;width:2px;background:var(--axis);transform:translateX(-50%);}
.pf-navbar-track{position:relative;flex:1;height:8px;border-radius:999px;background:var(--raised);}
.pf-navbar-fill{position:absolute;top:0;height:8px;border-radius:999px;}

@media(max-width:920px){
  .pf-thead,.pf-trow-d{display:none;}
  .pf-trow-m{display:block;padding:16px 4px;border-bottom:1px solid var(--border-soft);cursor:pointer;}
}

/* ---- evidence strip (navcoins lower edge) ---- */
.pf-evidence{margin-top:auto;border-top:1px solid var(--border-soft);padding-top:18px;margin-top:24px;
  display:flex;justify-content:space-between;align-items:center;gap:24px;flex-wrap:wrap;}
.pf-evidence-stats{display:flex;gap:30px;flex-wrap:wrap;}

/* ---- sheet ---- */
.pf-sheet-wrap{position:fixed;inset:0;background:rgba(0,0,0,0.6);display:flex;align-items:center;justify-content:center;z-index:50;padding:20px;}
.pf-sheet{width:100%;max-width:460px;background:var(--bg);border:1px solid var(--border);border-radius:18px;padding:22px;}
@media(max-width:560px){.pf-sheet-wrap{align-items:flex-end;padding:0;}.pf-sheet{max-width:none;border-radius:18px 18px 0 0;border-bottom:none;}}

.pf-progress{height:6px;border-radius:999px;background:var(--raised);overflow:hidden;}
.pf-progress-bar{height:100%;width:38%;border-radius:999px;background:var(--green);animation:pf-slide 1.25s ease-in-out infinite;}
@keyframes pf-slide{0%{margin-left:-38%}100%{margin-left:100%}}
@media(prefers-reduced-motion:reduce){.pf-progress-bar{animation:none;width:100%;}}
`;

/* ------------------------------ data ------------------------------- */

const NAVCOINS = [
  { id: "a651", name: "Reserve Alpha", nav: 1.0421, mkt: 1.0185, holding: 1250, supply: "4.03M", ledger: 4038983, hash: "0x9c4a…e71b" },
  { id: "b720", name: "Reserve Bravo", nav: 0.9884, mkt: 1.0122, holding: 0, supply: "1.88M", ledger: 4038971, hash: "0x21f0…aa3c" },
  { id: "c113", name: "Reserve Carbon", nav: 2.4513, mkt: 2.4559, holding: 480, supply: "0.92M", ledger: 4038990, hash: "0x7d12…04ff" },
  { id: "d904", name: "Reserve Delta", nav: 0.5121, mkt: 0.4982, holding: 0, supply: "6.41M", ledger: 4038955, hash: "0xbe88…39a1" },
];

const ACTIVITY = [
  { k: "Shielded swap", d: "pfUSDC → a651", v: "+1,250 a651", dir: "in", t: "2m ago" },
  { k: "Bridge", d: "USDC → pfUSDC", v: "+1,300 pfUSDC", dir: "in", t: "6m ago" },
  { k: "Account send", d: "to pf…91c4", v: "−40 PFT", dir: "out", t: "1h ago" },
  { k: "OTC buy", d: "c113 · Reserve Carbon", v: "+480 c113", dir: "in", t: "3h ago" },
  { k: "Verify proof", d: "a651 reserves", v: "confirmed", dir: "neutral", t: "3h ago" },
  { k: "Account send", d: "to pf…02ab", v: "−120 PFT", dir: "out", t: "yesterday" },
  { k: "Bridge", d: "USDC → pfUSDC", v: "+500 pfUSDC", dir: "in", t: "yesterday" },
];

const fmt = (n, d = 2) => n.toLocaleString("en-US", { minimumFractionDigits: d, maximumFractionDigits: d });
const premium = (c) => (c.mkt - c.nav) / c.nav; // <0 discount (below NAV), >0 premium
const marketValue = (c) => c.holding * c.mkt;
const navValue = (c) => c.holding * c.nav;

/* --------------------------- primitives ---------------------------- */

const Eyebrow = ({ children, style }) => <div className="pf-eyebrow" style={style}>{children}</div>;
const Pill = ({ children, tone }) => <span className={`pf-pill${tone ? " " + tone : ""}`}>{children}</span>;
const RowKV = ({ k, v }) => (<div className="pf-row"><span className="pf-rk">{k}</span><span className="pf-rv">{v}</span></div>);

function Stat({ label, value, color }) {
  return (
    <div>
      <Eyebrow style={{ fontSize: 10, marginBottom: 4 }}>{label}</Eyebrow>
      <div style={{ fontSize: 18, fontWeight: 650, color: color || "var(--text)" }}>{value}</div>
    </div>
  );
}

function PremiumBar({ c, height = 8, label = true, scale = 0.03 }) {
  const p = premium(c);
  const discount = p < 0;
  const neutral = Math.abs(p) < 0.002;
  const half = Math.min(Math.abs(p) / scale, 1) * 50;
  const color = neutral ? "var(--dim)" : discount ? "var(--green)" : "var(--amber)";
  return (
    <div className="pf-prem">
      <div className="pf-prem-track" style={{ height }}>
        <div className="pf-prem-axis" />
        <div className="pf-prem-bar" style={{ height, background: color, width: `${half}%`, left: discount ? `${50 - half}%` : "50%" }} />
      </div>
      {label && <span className="pf-prem-label" style={{ color }}>{discount ? "−" : "+"}{fmt(Math.abs(p) * 100, 1)}%</span>}
    </div>
  );
}

/* continuous-axis bar used inside the comparison table */
function NavBar({ c, scale = 0.03 }) {
  const p = premium(c);
  const discount = p < 0;
  const neutral = Math.abs(p) < 0.002;
  const half = Math.min(Math.abs(p) / scale, 1) * 50;
  const color = neutral ? "var(--dim)" : discount ? "var(--green)" : "var(--amber)";
  return (
    <div className="pf-navbar">
      <div className="pf-navbar-axis" />
      <div className="pf-navbar-track">
        <div className="pf-navbar-fill" style={{ background: color, width: `${half}%`, left: discount ? `${50 - half}%` : "50%" }} />
      </div>
    </div>
  );
}

/* ------------------------------ chrome ----------------------------- */

const NAV_ITEMS = [
  { id: "wallet", label: "Wallet" }, { id: "send", label: "Send" }, { id: "swap", label: "Swap" },
  { id: "nav", label: "NavCoins" }, { id: "more", label: "More" },
];
const isOn = (tab, id) => tab === id || (id === "nav" && tab === "navDetail");

function Sidebar({ tab, go, onLock }) {
  return (
    <aside className="pf-sidebar">
      <div className="pf-brand">
        <div className="pf-mark">PF</div>
        <div>
          <div style={{ fontWeight: 680, fontSize: 16, letterSpacing: "-0.02em" }}>PostFiat</div>
          <div style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--dim)" }}>self-custody</div>
        </div>
      </div>
      <button className="pf-chip" style={{ margin: "0 8px 14px" }} onClick={() => navigator.clipboard?.writeText("pfde0ba0...f74d0f")}>
        <span className="pf-dot" /> pfde0ba0…f74d0f
      </button>
      {NAV_ITEMS.map((x) => (
        <button key={x.id} className={`pf-nav${isOn(tab, x.id) ? " on" : ""}`} onClick={() => go(x.id)}>
          <span className="pf-nav-badge">{x.label[0]}</span>{x.label}
        </button>
      ))}
      <button className="pf-nav" onClick={onLock} style={{ color: "var(--dim)" }}>
        <span className="pf-nav-badge">L</span> Lock
      </button>
      <div className="pf-ledger">PostFiat WAN Devnet<br />ledger 4,038,983<br />49 / 52 validators</div>
    </aside>
  );
}

function TopBar({ onLock }) {
  return (
    <div className="pf-topbar">
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <div className="pf-mark" style={{ width: 30, height: 30, fontSize: 12 }}>PF</div>
        <div style={{ fontWeight: 680, fontSize: 15, letterSpacing: "-0.02em" }}>PostFiat</div>
      </div>
      <button className="pf-chip" onClick={onLock} style={{ fontSize: 11, letterSpacing: "0.08em" }}>LOCK</button>
    </div>
  );
}

function BottomNav({ tab, go }) {
  return (
    <nav className="pf-bottomnav">
      {NAV_ITEMS.map((x) => (
        <button key={x.id} className={`pf-bnav${isOn(tab, x.id) ? " on" : ""}`} onClick={() => go(x.id)}>
          <span className="pf-bnav-badge">{x.label[0]}</span>
          <span className="pf-bnav-l">{x.label}</span>
        </button>
      ))}
    </nav>
  );
}

/* ------------------------------ wallet ----------------------------- */

function WalletHome({ go }) {
  const held = NAVCOINS.filter((c) => c.holding > 0);
  const navMarket = held.reduce((s, c) => s + marketValue(c), 0);
  const best = [...NAVCOINS].sort((a, b) => premium(a) - premium(b))[0]; // widest discount

  return (
    <div className="pf-page fill">
      <div className="pf-band">
        <div>
          <Eyebrow>Total balance</Eyebrow>
          <div style={{ display: "flex", alignItems: "baseline", gap: 12, marginTop: 6 }}>
            <span style={{ fontSize: 58, fontWeight: 700, letterSpacing: "-0.045em", lineHeight: 1, color: "var(--green)" }}>2,840</span>
            <span style={{ fontFamily: "var(--mono)", fontSize: 15, color: "var(--muted)" }}>PFT</span>
          </div>
        </div>
        <div className="pf-actions">
          <button className="pf-ghost" onClick={() => navigator.clipboard?.writeText("pfde0ba0...f74d0f")}>Receive</button>
          <button className="pf-ghost" onClick={() => go("send")}>Send</button>
          <button className="pf-ghost" onClick={() => go("swap")}>Swap</button>
        </div>
      </div>

      {/* summary row — horizontal inhabitation, honest numbers only */}
      <div className="pf-stats">
        <div className="pf-tile">
          <Eyebrow style={{ fontSize: 10 }}>NavCoins · at market</Eyebrow>
          <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: "-0.02em" }}>${fmt(navMarket, 0)}</div>
          <div style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--dim)" }}>{held.length} funds held</div>
        </div>
        <button className="pf-tile" onClick={() => go("nav")}>
          <Eyebrow style={{ fontSize: 10 }}>Best opportunity</Eyebrow>
          <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: "-0.02em" }}>
            {best.id} <span style={{ color: "var(--green)", fontSize: 18 }}>−{fmt(Math.abs(premium(best)) * 100, 1)}%</span>
          </div>
          <div style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--dim)" }}>trading below NAV →</div>
        </button>
        <div className="pf-tile">
          <Eyebrow style={{ fontSize: 10 }}>Reserves</Eyebrow>
          <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: "-0.02em", color: "var(--mint)" }}>Verified</div>
          <div style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--dim)" }}>ledger 4,038,983</div>
        </div>
      </div>

      {/* body fills the remaining height; activity reaches the lower edge */}
      <div className="pf-dash">
        <div className="pf-dash-col">
          <div>
            <Eyebrow style={{ marginBottom: 12 }}>Balances</Eyebrow>
            <div className="pf-card" style={{ padding: "6px 18px" }}>
              {[["Account", "2,840 PFT", "Cobalt certified"], ["FastPay", "0 PFT", "owned objects"], ["pfUSDC", "1,300", "bridged from USDC"]]
                .map(([k, v, note], i, a) => (
                  <div key={k} className="pf-row" style={{ padding: "14px 0", borderBottom: i < a.length - 1 ? "1px solid var(--border-soft)" : "none" }}>
                    <div>
                      <div style={{ fontSize: 14, fontWeight: 600 }}>{k}</div>
                      <div style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--dim)" }}>{note}</div>
                    </div>
                    <div style={{ fontFamily: "var(--mono)", fontSize: 15 }}>{v}</div>
                  </div>
                ))}
            </div>
          </div>

          <div style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
            <div className="pf-row" style={{ marginBottom: 12 }}>
              <Eyebrow>Your NavCoins</Eyebrow>
              <button className="pf-link" onClick={() => go("nav")}>VIEW ALL →</button>
            </div>
            <div className="pf-card" style={{ padding: "6px 18px" }}>
              {held.map((c, i, a) => (
                <div key={c.id} onClick={() => go("navDetail", c.id)}
                  style={{ display: "grid", gridTemplateColumns: "1fr 1.5fr", gap: 18, alignItems: "center", padding: "15px 0", cursor: "pointer", borderBottom: i < a.length - 1 ? "1px solid var(--border-soft)" : "none" }}>
                  <div>
                    <div style={{ fontWeight: 650, fontSize: 15 }}>{c.id}</div>
                    {/* value at market (realizable) leads; NAV is muted context — figure and bar agree */}
                    <div style={{ fontFamily: "var(--mono)", fontSize: 12.5, marginTop: 2 }}>${fmt(marketValue(c), 0)}</div>
                    <div style={{ fontFamily: "var(--mono)", fontSize: 10.5, color: "var(--dim)" }}>NAV ${fmt(navValue(c), 0)}</div>
                  </div>
                  <PremiumBar c={c} />
                </div>
              ))}
            </div>
            <div style={{ marginTop: "auto", paddingTop: 16, fontFamily: "var(--mono)", fontSize: 11, color: "var(--dim)" }}>
              All held reserves attested on-ledger · last checked 3h ago
            </div>
          </div>
        </div>

        <div>
          <div className="pf-row" style={{ marginBottom: 12 }}>
            <Eyebrow>Recent activity</Eyebrow>
            <button className="pf-link" onClick={() => go("more")}>ALL →</button>
          </div>
          <div className="pf-card pf-activity-card" style={{ height: "calc(100% - 30px)" }}>
            <div className="pf-feed">
              {ACTIVITY.map((a, i) => (
                <div key={i} className="pf-act">
                  <div className="pf-act-l">
                    <div className="pf-act-t">{a.k}</div>
                    <div className="pf-act-s">{a.d} · {a.t}</div>
                  </div>
                  <div className="pf-act-v" style={{ color: a.dir === "in" ? "var(--mint)" : a.dir === "out" ? "var(--muted)" : "var(--dim)" }}>{a.v}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------- send ------------------------------ */

function Send() {
  const [lane, setLane] = useState("account");
  const [amt, setAmt] = useState("");
  const [to, setTo] = useState("");
  return (
    <div className="pf-page stage">
      <div className="pf-stage-inner" style={{ maxWidth: 900 }}>
        <Eyebrow>Send PFT</Eyebrow>
        <h1 className="pf-h1" style={{ marginBottom: 22 }}>{lane === "account" ? "Account lane" : "FastPay lane"}</h1>
        <div className="pf-two">
          <div style={{ display: "grid", gap: 16 }}>
            <div className="pf-even">
              <button className={`pf-ghost${lane === "account" ? " on" : ""}`} onClick={() => setLane("account")}>Account</button>
              <button className={`pf-ghost${lane === "fastpay" ? " on" : ""}`} onClick={() => setLane("fastpay")}>FastPay</button>
            </div>
            <div className="pf-card">
              <Eyebrow style={{ marginBottom: 10 }}>Amount</Eyebrow>
              <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
                <input value={amt} onChange={(e) => setAmt(e.target.value.replace(/[^0-9.]/g, ""))} placeholder="0" inputMode="decimal"
                  style={{ background: "transparent", border: "none", outline: "none", color: amt ? "var(--text)" : "var(--dim)", fontSize: 46, fontWeight: 700, letterSpacing: "-0.03em", width: "100%" }} />
                <span style={{ fontFamily: "var(--mono)", fontSize: 14, color: "var(--muted)" }}>PFT</span>
              </div>
            </div>
            <div>
              <Eyebrow style={{ marginBottom: 8 }}>Recipient</Eyebrow>
              <input className="pf-input" value={to} onChange={(e) => setTo(e.target.value)} placeholder="pf…" />
            </div>
            <button className="pf-primary" disabled={!amt || !to}>Review send</button>
          </div>

          <div className="pf-card" style={{ display: "grid", gap: 14, background: "var(--surface2)" }}>
            <Eyebrow>What happens</Eyebrow>
            <RowKV k="Lane" v={lane === "account" ? "Account" : "FastPay"} />
            <RowKV k="Settles in" v={lane === "account" ? "Cobalt finality · ~1.5s" : "Sub-second"} />
            <RowKV k="Visibility" v="Public on the explorer" />
            <RowKV k="Network fee" v="≈ 0.001 PFT" />
            <div style={{ borderTop: "1px solid var(--border-soft)", paddingTop: 12, fontSize: 13, color: "var(--muted)", lineHeight: 1.5 }}>
              {lane === "account"
                ? "The account lane carries the full balance and finalizes through Cobalt certification. Use it for any standard transfer."
                : "FastPay moves owned objects directly for near-instant settlement. Best for small, frequent payments."}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------- swap ------------------------------ */

const ROUTES = {
  private: { name: "Private", tag: "Shielded pool", why: "Default for shielded assets. Routed through the pool; the amount and pair stay hidden.", time: "≈ 30s", vis: "Private" },
  transparent: { name: "Transparent", tag: "On-ledger", why: "Public and fast — settles with Cobalt finality, visible on the explorer.", time: "≈ 1.5s", vis: "Public" },
  otc: { name: "OTC desk", tag: "PFTL · private", why: "Settle a NavCoin position with a desk. Best for size, no public footprint.", time: "quote", vis: "Private" },
  uniswap: { name: "Uniswap", tag: "Ethereum proxy", why: "Trade the proxy asset on Ethereum liquidity. Public, gas paid in ETH.", time: "≈ block", vis: "Public" },
};
function recommendRoute(from, to) {
  const priv = ["pfUSDC", "a651", "b720", "c113", "d904"];
  return priv.includes(from) || priv.includes(to) ? "private" : "transparent";
}

function Swap() {
  const assets = ["pfUSDC", "PFT", "a651", "b720", "c113", "d904"];
  const [from, setFrom] = useState("pfUSDC");
  const [to, setTo] = useState("a651");
  const [amt, setAmt] = useState("");
  const [route, setRoute] = useState(recommendRoute("pfUSDC", "a651"));
  const [changing, setChanging] = useState(false);
  const [phase, setPhase] = useState("idle");

  const setPair = (f, tt) => { setFrom(f); setTo(tt); setRoute(recommendRoute(f, tt)); };
  const r = ROUTES[route];
  const navTo = NAVCOINS.find((c) => c.id === to);
  const receive = amt && navTo ? `${fmt(parseFloat(amt) / navTo.nav, 2)} ${to}` : amt ? `${fmt(parseFloat(amt) * 0.998, 2)} ${to}` : `— ${to}`;
  const run = () => { setPhase("running"); setTimeout(() => setPhase("done"), 2600); };

  return (
    <div className="pf-page stage">
      <div className="pf-stage-inner" style={{ maxWidth: 980 }}>
        <Eyebrow>Swap</Eyebrow>
        <h1 className="pf-h1" style={{ marginBottom: 22 }}>Move between assets</h1>
        <div className="pf-two">
          <div className="pf-card" style={{ padding: 0, overflow: "hidden" }}>
            <AssetField label="From" value={from} onChange={(v) => setPair(v, to)} options={assets} amt={amt} setAmt={setAmt} />
            <div style={{ height: 1, background: "var(--border-soft)", position: "relative" }}>
              <button onClick={() => setPair(to, from)} aria-label="Swap direction"
                style={{ position: "absolute", left: "50%", top: "50%", transform: "translate(-50%,-50%)", width: 34, height: 34, borderRadius: 999, background: "var(--raised)", border: "1px solid var(--border)", color: "var(--mint)", cursor: "pointer", fontSize: 15 }}>↓</button>
            </div>
            <AssetField label="To" value={to} onChange={(v) => setPair(from, v)} options={assets} receive={receive} />
          </div>

          <div style={{ display: "grid", gap: 12 }}>
            <div className="pf-card" style={{ display: "grid", gap: 14, background: "var(--surface2)" }}>
              <div className="pf-row">
                <Eyebrow>Route</Eyebrow>
                <Pill tone={r.vis === "Private" ? "good" : undefined}>{r.tag}</Pill>
              </div>
              <div style={{ fontSize: 20, fontWeight: 680, letterSpacing: "-0.02em", color: r.vis === "Private" ? "var(--mint)" : "var(--text)" }}>{r.name}</div>
              <div style={{ fontSize: 13, color: "var(--muted)", lineHeight: 1.5, marginTop: -4 }}>{r.why}</div>
              <div style={{ borderTop: "1px solid var(--border-soft)", paddingTop: 12, display: "grid", gap: 9 }}>
                <RowKV k="You receive" v={`≈ ${receive}`} />
                <RowKV k="Settles in" v={r.time} />
                <RowKV k="Visibility" v={r.vis} />
              </div>
              <button className="pf-link" onClick={() => setChanging((s) => !s)} style={{ justifySelf: "start" }}>
                {changing ? "Hide routes ▴" : "Change route ▸"}
              </button>
              {changing && (
                <div style={{ display: "grid", gap: 6 }}>
                  {Object.entries(ROUTES).map(([id, rr]) => (
                    <button key={id} onClick={() => { setRoute(id); setChanging(false); }}
                      style={{ display: "flex", justifyContent: "space-between", alignItems: "center", textAlign: "left", padding: "11px 12px", borderRadius: 10, cursor: "pointer", background: route === id ? "var(--green-soft)" : "transparent", border: `1px solid ${route === id ? "var(--green-border)" : "var(--border)"}` }}>
                      <span style={{ fontSize: 13.5, fontWeight: 600, color: route === id ? "var(--mint)" : "var(--text)" }}>{rr.name}</span>
                      <span style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--dim)" }}>{rr.tag}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>

            {phase === "running" ? (
              <div className="pf-card" style={{ display: "grid", gap: 12 }}>
                <Eyebrow>Settling</Eyebrow>
                <div className="pf-progress"><div className="pf-progress-bar" /></div>
                <div style={{ fontSize: 13, color: "var(--muted)" }}>Keep the app open — this clears in a moment.</div>
              </div>
            ) : phase === "done" ? (
              <div className="pf-card" style={{ display: "grid", gap: 8, borderColor: "var(--green-border)", background: "var(--green-soft)" }}>
                <div style={{ fontWeight: 650, fontSize: 16, color: "var(--mint)" }}>Swap complete</div>
                <div style={{ fontFamily: "var(--mono)", fontSize: 12.5, color: "var(--muted)" }}>{amt || "0"} {from} → {receive}</div>
                <button className="pf-ghost" onClick={() => setPhase("idle")} style={{ marginTop: 6 }}>Done</button>
              </div>
            ) : (
              <button className="pf-primary" disabled={!amt} onClick={run}>
                {route === "uniswap" ? "Open on Uniswap" : route === "otc" ? "Request quote" : route === "private" ? "Swap privately" : "Swap"}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function AssetField({ label, value, onChange, options, amt, setAmt, receive }) {
  return (
    <div style={{ padding: 18 }}>
      <Eyebrow style={{ marginBottom: 10 }}>{label}</Eyebrow>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        {receive !== undefined ? (
          <span style={{ fontSize: 30, fontWeight: 700, letterSpacing: "-0.03em", color: "var(--dim)" }}>{receive.split(" ")[0] === "—" ? "≈ —" : "≈ " + receive.split(" ")[0]}</span>
        ) : (
          <input value={amt} onChange={(e) => setAmt(e.target.value.replace(/[^0-9.]/g, ""))} placeholder="0" inputMode="decimal"
            style={{ background: "transparent", border: "none", outline: "none", color: amt ? "var(--text)" : "var(--dim)", fontSize: 30, fontWeight: 700, letterSpacing: "-0.03em", width: "55%" }} />
        )}
        <select value={value} onChange={(e) => onChange(e.target.value)}
          style={{ background: "var(--raised)", color: "var(--text)", border: "1px solid var(--border)", borderRadius: 999, padding: "8px 12px", fontFamily: "var(--mono)", fontSize: 13, cursor: "pointer", outline: "none" }}>
          {options.map((o) => <option key={o} value={o} style={{ background: "var(--surface)" }}>{o}</option>)}
        </select>
      </div>
    </div>
  );
}

/* ---------------------------- navcoins ----------------------------- */

const SORTS = {
  dev: { get: premium, defDir: "asc", label: "Discount · Premium" }, // asc = widest discount first
  held: { get: (c) => c.holding, defDir: "desc" },
  nav: { get: (c) => c.nav, defDir: "desc" },
  mkt: { get: (c) => c.mkt, defDir: "desc" },
};

function NavList({ go }) {
  const [sort, setSort] = useState({ key: "dev", dir: "asc" });
  const click = (key) => setSort((s) => (s.key === key ? { key, dir: s.dir === "asc" ? "desc" : "asc" } : { key, dir: SORTS[key].defDir }));
  const caret = (key) => (sort.key === key ? (sort.dir === "asc" ? " ↑" : " ↓") : "");
  const rows = [...NAVCOINS].sort((a, b) => {
    const d = SORTS[sort.key].get(a) - SORTS[sort.key].get(b);
    return sort.dir === "asc" ? d : -d;
  });

  return (
    <div className="pf-page fill">
      <Eyebrow>Proof-of-reserves funds</Eyebrow>
      <h1 className="pf-h1">NavCoins</h1>
      <p style={{ fontSize: 13.5, color: "var(--muted)", lineHeight: 1.55, marginTop: 10, maxWidth: 600 }}>
        Shielded funds with verifiable reserves. Each bar is positioned against the NAV line — the brighter
        vertical axis is at NAV, so a green bar to the left is trading below NAV. Sorted by deepest discount.
      </p>

      <div className="pf-card" style={{ padding: 0, marginTop: 18 }}>
        <div className="pf-thead">
          <span className="pf-th" style={{ cursor: "default" }}>Fund</span>
          <button className={`pf-th r${sort.key === "held" ? " active" : ""}`} onClick={() => click("held")}>Held{caret("held")}</button>
          <button className={`pf-th r${sort.key === "nav" ? " active" : ""}`} onClick={() => click("nav")}>NAV / unit{caret("nav")}</button>
          <button className={`pf-th r${sort.key === "mkt" ? " active" : ""}`} onClick={() => click("mkt")}>Market{caret("mkt")}</button>
          <button className={`pf-th c${sort.key === "dev" ? " active" : ""}`} onClick={() => click("dev")}>Discount ← NAV → Premium{caret("dev")}</button>
          <span className="pf-th r" style={{ cursor: "default" }} />
          <span />
        </div>

        {rows.map((c) => (
          <React.Fragment key={c.id}>
            <div className="pf-trow-d" onClick={() => go("navDetail", c.id)}>
              <div>
                <span style={{ fontWeight: 700, fontSize: 16, letterSpacing: "-0.01em" }}>{c.id}</span>
                <div style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--dim)", marginTop: 2 }}>{c.name}</div>
              </div>
              <div className="pf-num" style={{ color: c.holding ? "var(--text)" : "var(--dim)" }}>{c.holding ? fmt(c.holding, 0) : "—"}</div>
              <div className="pf-num">${fmt(c.nav, 4)}</div>
              <div className="pf-num" style={{ color: "var(--muted)" }}>${fmt(c.mkt, 4)}</div>
              <NavBar c={c} />
              <div className="pf-num" style={{ color: premium(c) < 0 ? "var(--green)" : Math.abs(premium(c)) < 0.002 ? "var(--dim)" : "var(--amber)" }}>
                {premium(c) < 0 ? "−" : "+"}{fmt(Math.abs(premium(c)) * 100, 1)}%
              </div>
              <span style={{ color: "var(--dim)", textAlign: "right" }}>→</span>
            </div>

            <div className="pf-trow-m" onClick={() => go("navDetail", c.id)}>
              <div className="pf-row" style={{ marginBottom: 4 }}>
                <span style={{ fontWeight: 700, fontSize: 16 }}>{c.id}</span>
                {c.holding > 0 && <Pill>{fmt(c.holding, 0)} held</Pill>}
              </div>
              <div style={{ fontFamily: "var(--mono)", fontSize: 11.5, color: "var(--dim)", marginBottom: 10 }}>
                NAV ${fmt(c.nav, 4)} · Market ${fmt(c.mkt, 4)}
              </div>
              <PremiumBar c={c} height={9} />
            </div>
          </React.Fragment>
        ))}
      </div>

      {/* designed lower edge: live reserve evidence, echoing the landing page's VHS panel */}
      <div className="pf-evidence">
        <div>
          <Eyebrow>Live reserve evidence</Eyebrow>
          <div style={{ fontSize: 13, color: "var(--muted)", marginTop: 6 }}>All fund reserves attested on-ledger and independently verifiable.</div>
        </div>
        <div className="pf-evidence-stats">
          <Stat label="Ledger" value="4,038,983" />
          <Stat label="Verified domains" value="38 / 40" />
          <Stat label="24h agreement" value="99.9%+" color="var(--mint)" />
        </div>
      </div>
    </div>
  );
}

function NavDetail({ id, go }) {
  const c = NAVCOINS.find((x) => x.id === id) || NAVCOINS[0];
  const [verified, setVerified] = useState(false);
  const [sheet, setSheet] = useState(null);
  const p = premium(c);
  const discount = p < 0;
  const color = discount ? "var(--green)" : "var(--amber)";

  return (
    <div className="pf-page">
      <button className="pf-link" onClick={() => go("nav")} style={{ fontSize: 12, marginBottom: 14, alignSelf: "start" }}>← NavCoins</button>
      <div className="pf-two">
        <div style={{ display: "grid", gap: 20 }}>
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <h1 className="pf-h1" style={{ marginTop: 0 }}>{c.id}</h1>
              {c.holding > 0 && <Pill>{fmt(c.holding, 0)} held</Pill>}
            </div>
            <div style={{ fontFamily: "var(--mono)", fontSize: 12, color: "var(--dim)", marginTop: 4 }}>{c.name}</div>
          </div>

          <div className="pf-card" style={{ display: "grid", gap: 18 }}>
            <div className="pf-row" style={{ alignItems: "flex-end" }}>
              <div>
                <Eyebrow>{discount ? "Trading below NAV" : "Trading above NAV"}</Eyebrow>
                <div style={{ fontSize: 48, fontWeight: 700, letterSpacing: "-0.045em", lineHeight: 1.05, color, marginTop: 4 }}>
                  {discount ? "−" : "+"}{fmt(Math.abs(p) * 100, 1)}%
                </div>
              </div>
              <Eyebrow style={{ fontSize: 10 }}>{discount ? "buy below NAV" : "premium to NAV"}</Eyebrow>
            </div>
            <PremiumBar c={c} height={12} label={false} />
            <div style={{ display: "flex", justifyContent: "space-between", fontFamily: "var(--mono)", fontSize: 10, color: "var(--dim)", marginTop: -8 }}>
              <span>← discount</span><span>NAV</span><span>premium →</span>
            </div>
            <div style={{ display: "flex", gap: 36, borderTop: "1px solid var(--border-soft)", paddingTop: 16 }}>
              <Stat label="NAV / unit" value={`$${fmt(c.nav, 4)}`} />
              <Stat label="Uniswap price" value={`$${fmt(c.mkt, 4)}`} color="var(--muted)" />
            </div>
          </div>

          {c.holding > 0 && (
            <div className="pf-card" style={{ display: "grid", gap: 10 }}>
              <Eyebrow>Your position</Eyebrow>
              <RowKV k="Holding" v={`${fmt(c.holding, 0)} ${c.id}`} />
              <RowKV k="Value at market" v={`$${fmt(marketValue(c), 2)}`} />
              <RowKV k="Value at NAV" v={`$${fmt(navValue(c), 2)}`} />
            </div>
          )}

          <div className="pf-even">
            <button className="pf-primary" onClick={() => setSheet("buy")}>Buy {c.id}</button>
            <button className="pf-ghost" onClick={() => setSheet("sell")}>Sell</button>
          </div>
          <button className="pf-ghost" onClick={() => go("swap")}>Swap into another NavCoin →</button>
        </div>

        <div className="pf-card" style={{ display: "grid", gap: 16, borderColor: verified ? "var(--green-border)" : "var(--border)", background: "var(--surface2)" }}>
          <div className="pf-row">
            <Eyebrow>Proof of reserves</Eyebrow>
            {verified && <Pill tone="good">VERIFIED</Pill>}
          </div>
          <div style={{ display: "grid", gap: 11 }}>
            <RowKV k="Reserve attestation" v={c.hash} />
            <RowKV k="Ledger height" v={fmt(c.ledger, 0)} />
            <RowKV k="Outstanding supply" v={c.supply} />
            <RowKV k="Backing" v="1.00× shielded reserve" />
          </div>
          <div style={{ fontSize: 12.5, color: "var(--muted)", lineHeight: 1.5, borderTop: "1px solid var(--border-soft)", paddingTop: 12 }}>
            Reserves are held in a shielded fund and attested on-ledger. Verify to confirm the backing asset against the published height yourself.
          </div>
          <button className={`pf-ghost${verified ? " on" : ""}`} onClick={() => setVerified(true)}>
            {verified ? "Reserves confirmed on-ledger ✓" : "Verify proof"}
          </button>
        </div>
      </div>
      {sheet && <RouteSheet kind={sheet} coin={c} onClose={() => setSheet(null)} />}
    </div>
  );
}

function RouteSheet({ kind, coin, onClose }) {
  const discount = premium(coin) < 0;
  const recommended = discount ? "uniswap" : "otc"; // if it's cheaper on the public market, recommend that
  const [choice, setChoice] = useState(recommended);
  const routes = [
    { id: "otc", name: "OTC desk · PFTL", tag: "Private", detail: "Settle privately with a desk on PostFiat. No public footprint." },
    { id: "uniswap", name: "Uniswap · Ethereum", tag: "Public", detail: `Trade the proxy asset on Ethereum — market is ${discount ? "below" : "above"} NAV right now.` },
  ];
  return (
    <div className="pf-sheet-wrap" onClick={onClose}>
      <div className="pf-sheet" onClick={(e) => e.stopPropagation()} style={{ display: "grid", gap: 14 }}>
        <Eyebrow>{kind === "buy" ? "Buy" : "Sell"} {coin.id}</Eyebrow>
        <div style={{ fontSize: 13, color: "var(--muted)", marginTop: -6 }}>
          {kind === "buy" && discount
            ? "Recommended: Uniswap — it's trading below NAV, so the public market is the cheaper entry right now."
            : "Recommended: the private OTC desk. Switch to Uniswap to use Ethereum liquidity instead."}
        </div>
        {routes.map((r) => (
          <button key={r.id} onClick={() => setChoice(r.id)}
            style={{ textAlign: "left", background: choice === r.id ? "var(--green-soft)" : "var(--surface)", border: `1px solid ${choice === r.id ? "var(--green-border)" : "var(--border)"}`, borderRadius: 14, padding: "14px 15px", cursor: "pointer" }}>
            <div className="pf-row">
              <span style={{ fontWeight: 650, fontSize: 15, color: choice === r.id ? "var(--mint)" : "var(--text)" }}>{r.name}</span>
              <Pill tone={r.id === "otc" ? "good" : undefined}>{r.tag}{r.id === recommended ? " · recommended" : ""}</Pill>
            </div>
            <div style={{ fontSize: 12.5, color: "var(--muted)", marginTop: 6, lineHeight: 1.45 }}>{r.detail}</div>
          </button>
        ))}
        <button className="pf-primary" onClick={onClose}>{kind === "buy" ? "Continue to buy" : "Continue to sell"}</button>
      </div>
    </div>
  );
}

/* ------------------------------- more ------------------------------ */

function More() {
  return (
    <div className="pf-page stage">
      <div className="pf-stage-inner" style={{ maxWidth: 980 }}>
        <Eyebrow>Settings</Eyebrow>
        <h1 className="pf-h1" style={{ marginBottom: 22 }}>More</h1>
        <div className="pf-even" style={{ alignItems: "start" }}>
          <div className="pf-card" style={{ display: "grid", gap: 16 }}>
            <Eyebrow>Network</Eyebrow>
            <Field label="RPC endpoint"><Select options={["PostFiat WAN Devnet", "PostFiat Mainnet", "Local node"]} /></Field>
            <Field label="Swap server"><Input value="http://localhost:8787" /></Field>
            <Field label="Bridge vault contract"><Input placeholder="0x…" /></Field>
          </div>
          <div className="pf-card" style={{ display: "grid", gap: 16 }}>
            <Eyebrow>Wallet</Eyebrow>
            <Field label="Auto-lock (minutes)"><Select options={["5", "15", "30", "Never"]} /></Field>
            <button className="pf-primary">Save settings</button>
            <div className="pf-even"><button className="pf-ghost">Export backup</button><button className="pf-ghost">Import backup</button></div>
            <button style={{ width: "100%", background: "var(--red-soft)", border: "1px solid rgba(239,106,106,0.3)", color: "var(--red)", borderRadius: 12, padding: 14, fontSize: 14, fontWeight: 600, cursor: "pointer" }}>Remove wallet</button>
          </div>
        </div>
      </div>
    </div>
  );
}

const Field = ({ label, children }) => (
  <div style={{ display: "grid", gap: 7 }}>
    <span style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--dim)", letterSpacing: "0.06em", textTransform: "uppercase" }}>{label}</span>
    {children}
  </div>
);
const Input = ({ value, placeholder }) => { const [v, setV] = useState(value || ""); return <input className="pf-input" value={v} placeholder={placeholder} onChange={(e) => setV(e.target.value)} />; };
const Select = ({ options }) => <select className="pf-select">{options.map((o) => <option key={o} style={{ background: "var(--surface)" }}>{o}</option>)}</select>;

/* ------------------------------ locked ----------------------------- */

function Locked({ onUnlock }) {
  return (
    <div style={{ display: "grid", placeItems: "center", height: "100vh", padding: 24, gap: 20 }}>
      <div className="pf-mark" style={{ width: 56, height: 56, borderRadius: 16, fontSize: 18 }}>PF</div>
      <div style={{ textAlign: "center" }}>
        <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: "-0.02em" }}>Wallet locked</div>
        <div style={{ fontFamily: "var(--mono)", fontSize: 12, color: "var(--dim)", marginTop: 6 }}>self-custody · pfde0ba0…f74d0f</div>
      </div>
      <button className="pf-primary" onClick={onUnlock} style={{ maxWidth: 240 }}>Unlock</button>
    </div>
  );
}

/* ------------------------------- app ------------------------------- */

export default function PostFiatWallet() {
  const [tab, setTab] = useState("wallet");
  const [coinId, setCoinId] = useState("a651");
  const [locked, setLocked] = useState(false);
  const go = (next, payload) => { if (next === "navDetail") setCoinId(payload); setTab(next); };

  return (
    <div className="pf-root">
      <style>{STYLE}</style>
      {locked ? (
        <Locked onUnlock={() => setLocked(false)} />
      ) : (
        <div className="pf-shell">
          <Sidebar tab={tab} go={go} onLock={() => setLocked(true)} />
          <main className="pf-main">
            <TopBar onLock={() => setLocked(true)} />
            {tab === "wallet" && <WalletHome go={go} />}
            {tab === "send" && <Send />}
            {tab === "swap" && <Swap />}
            {tab === "nav" && <NavList go={go} />}
            {tab === "navDetail" && <NavDetail id={coinId} go={go} />}
            {tab === "more" && <More />}
            <BottomNav tab={tab} go={go} />
          </main>
        </div>
      )}
    </div>
  );
}
