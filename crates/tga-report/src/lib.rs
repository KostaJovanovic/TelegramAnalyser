//! Assemble one self-contained `report.html`.
//!
//! Self-contained means exactly that: one file, no folder beside it, no request
//! to anything. The fonts are base64'd into the stylesheet, the charts are
//! inline SVG, and the only script is the twenty lines that run the tooltips
//! and the two switches. An archive is something you keep, and a report that
//! needs a CDN to render is a report that stops working the year the CDN does.
//!
//! The visual language is Swiss/International: hairlines do the dividing,
//! corners are square, numbers are set in Geist Mono, and section headings are
//! letterspaced uppercase micro-type. There are no cards.
//!
//! **This crate depends on neither `tga-read` nor `tga-metrics.`** It renders
//! from a `serde_json::Value` of the shape `--stats` dumps, which is what lets
//! `tests/golden.rs` replay a recorded fixture through the writer with no
//! export on disk. See `Cargo.toml`.

use serde_json::Value;
use tga_notes::Notes;

pub mod assets;
pub mod charts;
pub mod palette;
pub mod sections;
pub mod stats;

pub use charts::esc;
pub use sections::{Names, WIDTH};

// ---------------------------------------------------------------------------
// the stylesheet
// ---------------------------------------------------------------------------

/// The custom properties, as one `<html>` class block.
///
/// **The report is dark only.** One block, and no control to switch away from
/// it: a second appearance is a second design to keep in step, and this one has
/// two colours and a red to keep in step already.
fn tokens_css() -> String {
    let pairs: String = palette::tokens()
        .iter()
        .map(|(k, v)| format!("--{k}:{v};"))
        .collect();
    let ramp: String = palette::ramp()
        .iter()
        .enumerate()
        .map(|(i, c)| format!("--r{}:{c};", i + 1))
        .collect();
    format!(
        "html.dark{{{pairs}{ramp}--self:{};}}",
        palette::token("rule")
    )
}

/// The stylesheet.
///
/// **The `\u{91}2` in the `details[open]` marker is a defect**, inherited from
/// the program this one was ported from: a C1 control character followed by an
/// ASCII `2`, which is what a `−` becomes after one bad encoding round trip, and
/// which a browser renders as a stray `2` on an open disclosure. It survived
/// this long because reproducing it kept a byte diff at zero, and that diff is
/// now gone. [`CSS_SURFACE`] overrides it with a real minus; the two are merged
/// and this literal deleted in the commit that moves the stylesheet out into its
/// own file, which is the one place the change is reviewable as a change rather
/// than as noise inside a larger move.
const CSS: &str = concat!(
    r#"
*,*::before,*::after{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{
  margin:0 auto;max-width:1180px;padding:0 30px 200px;
  background:var(--bg);color:var(--fg);
  font-family:'Geist','Segoe UI',system-ui,sans-serif;
  font-size:14px;line-height:1.62;
  -webkit-font-smoothing:antialiased;text-rendering:optimizeLegibility;
}
a{color:inherit;text-decoration:none;border-bottom:1px solid var(--rule)}
a:hover{border-bottom-color:var(--accent)}
:focus-visible{outline:2px solid var(--accent);outline-offset:2px}

/* -- type ---------------------------------------------------------------- */
.eyebrow,h2,th,.switch,.toc a,.datatoggle{
  font-family:'Geist Mono',ui-monospace,Consolas,monospace;
  font-size:10px;font-weight:400;text-transform:uppercase;letter-spacing:.18em;
}
.eyebrow{color:var(--muted);margin:0 0 18px}
h1{
  font-size:clamp(34px,6vw,54px);font-weight:500;letter-spacing:-.028em;
  line-height:1.02;margin:0 0 14px;
}
h2{color:var(--muted);margin:0}
h3{font-size:15px;font-weight:500;letter-spacing:-.01em;margin:38px 0 4px}
p{margin:0 0 14px;max-width:74ch}
.lede{font-size:16px;color:var(--muted);max-width:62ch;margin:0 0 30px}
.note{font-size:12.5px;color:var(--muted);max-width:74ch;margin:8px 0 0}
.num,td.n,th.n,.figures b,.value,.axis,.rowlabel,.nodelabel{
  font-family:'Geist Mono',ui-monospace,Consolas,monospace;
  font-variant-numeric:tabular-nums;
}

/* -- masthead ------------------------------------------------------------ */
.masthead{padding:76px 0 0}
.toc{display:flex;flex-wrap:wrap;gap:0;margin:34px 0 0;
  border-top:1px solid var(--rule);border-bottom:1px solid var(--rule)}
.toc a{padding:11px 16px 10px;border:0;border-right:1px solid var(--rule);color:var(--muted)}
.toc a:first-child{padding-left:0}
.toc a:hover{color:var(--fg)}
.switches{display:flex;gap:0;margin-top:0;border-bottom:1px solid var(--rule)}
.switch{
  background:none;border:0;border-right:1px solid var(--rule);
  color:var(--muted);padding:10px 16px;cursor:pointer;
}
.switch:first-child{padding-left:0}
.switch:hover{color:var(--fg)}
.switch[aria-pressed="true"]{color:var(--accent)}

/* -- sections ------------------------------------------------------------ */
section{padding-top:74px}
.sechead{
  display:flex;align-items:baseline;justify-content:space-between;gap:20px;
  padding-bottom:9px;border-bottom:1px solid var(--hairline);margin-bottom:26px;
}
.sechead .count{
  font-family:'Geist Mono',ui-monospace,monospace;font-size:11px;
  color:var(--muted);font-variant-numeric:tabular-nums;
}

/* -- figures: hairline-separated, never boxed ---------------------------- */
.figures{display:flex;flex-wrap:wrap;list-style:none;margin:30px 0 0;padding:0}
.figures li{
  padding:2px 26px 2px 26px;border-left:1px solid var(--rule);
  min-width:118px;flex:0 0 auto;
}
.figures li:first-child{padding-left:0;border-left:0}
.figures b{display:block;font-size:25px;font-weight:400;letter-spacing:-.02em;line-height:1.15}
.figures span{display:block;font-size:11.5px;color:var(--muted);margin-top:3px}
.figures em{font-style:normal;color:var(--muted);font-size:15px}
.figures.tight li{min-width:0;padding:2px 22px}
.figures.tight b{font-size:22px;line-height:1.3}

/* -- charts -------------------------------------------------------------- */
svg.chart{display:block;width:100%;height:auto;overflow:visible}
.ribbon-bar{fill:var(--accent)}
.ribbon-bed{fill:var(--rule)}
.ribbon-bar:hover{fill:var(--fg)}
.hero .ribbon-bar{fill:var(--accent)}
.mini .ribbon-bar{fill:var(--accent)}
.mini .ribbon-bed{fill:transparent}
.col,.bar{fill:var(--accent)}
.col:hover,.bar:hover,.cell:hover{fill:var(--fg)}
.baseline,.tick{stroke:var(--rule);stroke-width:1}
.axis{font-size:9.5px;fill:var(--muted);letter-spacing:.08em}
.rowlabel{font-size:11px;fill:var(--fg)}
.value{font-size:11px;fill:var(--muted)}
.cell{shape-rendering:crispEdges}
.link{stroke:var(--accent)}
.dot{fill:var(--accent);stroke:var(--bg);stroke-width:2}
.nodelabel{font-size:9.5px;fill:var(--muted)}
.colhead{text-anchor:start}
.legend{display:flex;align-items:center;gap:4px;font-size:11px;color:var(--muted);margin:12px 0 0}
.legend i{width:22px;height:9px;display:inline-block}
.legend span{margin:0 6px}
.swatch-track{background:var(--track)}
.caption{font-size:11.5px;color:var(--muted);margin:10px 0 0}

/* -- events -------------------------------------------------------------- */
.ev-dot{fill:var(--accent);stroke:var(--bg);stroke-width:2}
.ev-hit{fill:transparent}
.ev-stem{stroke:var(--rule);stroke-width:1}
.ev-span{fill:var(--accent);opacity:.45}
.ev{cursor:pointer}
.ev:hover .ev-dot,.ev:focus .ev-dot{fill:var(--fg)}
.ev.conf-low .ev-dot{fill:var(--bg);stroke:var(--accent)}
.ev.on .ev-dot{fill:var(--fg)}
.events{list-style:none;margin:26px 0 0;padding:0}
.events li{padding:16px 0;border-top:1px solid var(--rule);display:flex;gap:24px}
.events li.on{border-left:2px solid var(--accent);padding-left:16px;margin-left:-18px}
.events time{
  font-family:'Geist Mono',ui-monospace,monospace;font-size:11px;color:var(--muted);
  flex:0 0 118px;padding-top:2px;
}
.events h4{margin:0 0 4px;font-size:15px;font-weight:500;letter-spacing:-.01em}
.events p{margin:0;font-size:13px;color:var(--muted)}
.events .cite{
  font-family:'Geist Mono',ui-monospace,monospace;font-size:10px;
  letter-spacing:.1em;text-transform:uppercase;color:var(--muted);margin-top:7px;
}
.events .kind{color:var(--accent)}

/* -- tables -------------------------------------------------------------- */
table{border-collapse:collapse;width:100%;margin:4px 0 0}
th{
  color:var(--muted);text-align:left;padding:0 0 9px;
  border-bottom:1px solid var(--hairline);white-space:nowrap;
}
td{padding:9px 0;border-bottom:1px solid var(--rule);vertical-align:middle}
th+th,td+td{padding-left:26px}
td.n,th.n{text-align:right}
.rank{
  font-family:'Geist Mono',ui-monospace,monospace;font-variant-numeric:tabular-nums;
  text-align:right;color:var(--muted);width:34px;
}
tbody tr:hover td{background:var(--surface)}
td.name{font-weight:500}
.alias{display:none;color:var(--muted);font-weight:400;font-size:12px}
html.aliases .alias{display:inline}
.presence{width:31%}
.presence svg{height:15px}
td.name{min-width:186px}
.role{font-size:10px;letter-spacing:.12em;text-transform:uppercase;color:var(--accent)}
tr.overflow{display:none}
html.everyone tr.overflow{display:table-row}
.datatoggle{
  background:none;border:0;color:var(--muted);cursor:pointer;padding:10px 0 0;
}
.datatoggle:hover{color:var(--fg)}
details.data{margin:14px 0 0}
details.data summary{
  font-family:'Geist Mono',ui-monospace,monospace;font-size:10px;
  text-transform:uppercase;letter-spacing:.18em;color:var(--muted);
  cursor:pointer;list-style:none;padding:9px 0;border-top:1px solid var(--rule);
}
details.data summary::before{content:'+ ';color:var(--accent)}
details.data[open] summary::before{content:'"#,
    "\u{91}",
    r#"2 '}
details.data summary::-webkit-details-marker{display:none}
details.data summary:hover{color:var(--fg)}
details.data[open] summary{color:var(--fg)}

/* -- record list --------------------------------------------------------- */
.records{list-style:none;margin:0;padding:0}
.records li{
  display:flex;gap:24px;align-items:baseline;
  padding:15px 0;border-bottom:1px solid var(--rule);
}
.records .what{flex:0 0 168px;color:var(--muted);font-size:12px}
.records .big{
  font-family:'Geist Mono',ui-monospace,monospace;font-size:20px;
  flex:0 0 118px;font-variant-numeric:tabular-nums;
}
.records .about{flex:1;font-size:13px}
.records .about span{color:var(--muted)}

.cols2{display:grid;grid-template-columns:1fr 1fr;gap:0 56px}
.split{display:grid;grid-template-columns:minmax(0,1fr) 236px;gap:0 44px;align-items:end}
.split aside{padding-bottom:6px}
.split aside .caption{margin-top:14px}
@media (max-width:820px){
  body{padding:0 18px 120px}
  .cols2,.split{grid-template-columns:1fr}
  .presence{display:none}
  .figures li{padding:2px 18px;min-width:96px}
}

/* -- tooltip ------------------------------------------------------------- */
#tip{
  position:fixed;pointer-events:none;z-index:40;opacity:0;
  background:var(--fg);color:var(--bg);
  font-family:'Geist Mono',ui-monospace,monospace;font-size:11px;
  padding:5px 8px;white-space:nowrap;transform:translate(-50%,-150%);
}
#tip.on{opacity:1}
@media print{.switches,.toc,#tip{display:none}body{max-width:none}}
"#
);

/// The second half of the stylesheet, still appended rather than merged.
///
/// It was split so that the frozen render could omit it. That render is gone
/// and the split has no reason left; the two are merged when the stylesheet
/// moves out into its own file, and kept apart until then only so that the
/// commit which deleted the frozen render changed no bytes at all.
// NOTE: the `/* */` blocks in this string are *stylesheet* comments -- they are
// emitted into the report, so editing one changes the file's bytes. They are
// left exactly as they were until the merge that deletes them.
const CSS_SURFACE: &str = r#"
/* -- the one thing here that overrides rather than adds ------------------- */
/* `report.py` ships `content:'<U+0091>2 '` on this rule -- a C1 control
   character followed by an ASCII 2, which is what a minus sign becomes after a
   bad encoding round trip, and which every browser renders as a stray "2" on an
   open disclosure. The classic path reproduces it so the parity diff stays a
   clean zero. Here there is no diff to answer to, so it is simply fixed: this
   rule has equal specificity and comes later, so it wins. */
details.data[open] summary::before{content:'\2212 '}

/* -- search -------------------------------------------------------------- */
.find{
  background:none;border:0;border-right:1px solid var(--rule);color:var(--fg);
  font-family:'Geist Mono',ui-monospace,monospace;font-size:10px;
  text-transform:uppercase;letter-spacing:.18em;
  padding:10px 16px;min-width:22ch;flex:1;outline:0;
}
.find::placeholder{color:var(--muted)}
.find:focus{border-bottom:1px solid var(--accent)}
.found{
  font-family:'Geist Mono',ui-monospace,monospace;font-size:10px;
  letter-spacing:.18em;color:var(--muted);align-self:center;padding:0 16px;
}
.nomatch{display:none !important}
html.searching tr.overflow{display:table-row}

/* -- compact presence rows ----------------------------------------------- */
/* One path per shade, so a row is at most five elements however long the
   archive is. The fill moves out of every cell and into these five rules. */
.strip .c1{fill:var(--r1)}
.strip .c2{fill:var(--r2)}
.strip .c3{fill:var(--r3)}
.strip .c4{fill:var(--r4)}
.strip .c5{fill:var(--r5)}
.strip{cursor:crosshair}

/* -- coverage ------------------------------------------------------------ */
.cov-bed{fill:var(--rule)}
.cov-read{fill:var(--accent)}
.coverage{
  font-family:'Geist Mono',ui-monospace,monospace;font-size:11px;
  color:var(--muted);margin:6px 0 0;letter-spacing:.02em;
}
.coverage b{color:var(--fg);font-weight:400}

/* -- kind filters -------------------------------------------------------- */
.kinds{display:flex;flex-wrap:wrap;gap:0;margin:26px 0 0;
  border-top:1px solid var(--rule);border-bottom:1px solid var(--rule)}
.kind-filter{
  background:none;border:0;border-right:1px solid var(--rule);cursor:pointer;
  color:var(--muted);padding:10px 16px;
  font-family:'Geist Mono',ui-monospace,monospace;font-size:10px;
  text-transform:uppercase;letter-spacing:.18em;
}
.kind-filter:first-child{padding-left:0}
.kind-filter i{font-style:normal;margin-right:8px}
.kind-filter span{margin-left:8px;font-variant-numeric:tabular-nums}
.kind-filter:hover{color:var(--fg)}
.kind-filter[aria-pressed="true"]{color:var(--fg)}
.kind-filter[aria-pressed="true"] i{color:var(--accent)}
.kind-filter[aria-pressed="false"]{opacity:.45}

/* -- the fields the _timeline layout carries ----------------------------- */
.events time .at{display:block;color:var(--muted);opacity:.75}
.events .w{
  font-family:'Geist Mono',ui-monospace,monospace;font-size:9.5px;
  text-transform:uppercase;letter-spacing:.14em;color:var(--muted);
  margin-left:10px;vertical-align:2px;
}
.events .w-major{color:var(--accent)}
.events .who{margin:7px 0 0;font-size:12px;color:var(--muted)}
.events .who span{margin-right:12px;color:var(--fg)}
.events .who span.unknown{color:var(--muted);text-decoration:underline dotted}
.events .who em{font-style:normal;color:var(--accent);font-size:11px}
.events .tags{margin:6px 0 0}
.events .tags span{
  font-family:'Geist Mono',ui-monospace,monospace;font-size:10px;
  letter-spacing:.1em;color:var(--muted);margin-right:14px;
}
.events .tags span::before{content:'#';color:var(--rule)}
@media (max-width:820px){.find{min-width:0}}
"#;

/// The whole of the interactivity: two class switches, a tooltip, and the
/// event selection.
///
/// Every view on the page is already rendered into the SVG; this only filters
/// and highlights what is drawn. A report that renders blank with scripting off
/// has stopped being an archive.
const JS: &str = r#"
(function(){
  var root=document.documentElement;
  function setTheme(name){
    root.classList.remove('light','dark');root.classList.add(name);
    var b=document.getElementById('theme');
    if(b){b.textContent=name==='dark'?'Light':'Dark';}
    try{localStorage.setItem('tg-report-theme',name);}catch(e){}
  }
  try{var saved=localStorage.getItem('tg-report-theme');if(saved)setTheme(saved);}catch(e){}
  var themeBtn=document.getElementById('theme');
  if(themeBtn)themeBtn.addEventListener('click',function(){
    setTheme(root.classList.contains('dark')?'light':'dark');
  });
  function toggle(id,cls){
    var b=document.getElementById(id);
    if(!b)return;
    b.addEventListener('click',function(){
      var on=root.classList.toggle(cls);
      b.setAttribute('aria-pressed',on?'true':'false');
    });
  }
  toggle('aliases','aliases');
  toggle('everyone','everyone');

  var tip=document.getElementById('tip');
  function show(el,x,y){
    var text=el.getAttribute('data-tip');
    if(!text)return;
    tip.textContent=text;tip.classList.add('on');
    tip.style.left=x+'px';tip.style.top=y+'px';
  }
  document.addEventListener('mousemove',function(e){
    var el=e.target.closest?e.target.closest('[data-tip]'):null;
    if(el){show(el,e.clientX,e.clientY);}else{tip.classList.remove('on');}
  });
  document.addEventListener('focusin',function(e){
    var el=e.target.closest?e.target.closest('[data-tip]'):null;
    if(!el){tip.classList.remove('on');return;}
    var r=el.getBoundingClientRect();show(el,r.left+r.width/2,r.top);
  });

  function selectEvent(id){
    document.querySelectorAll('.ev.on,.events li.on').forEach(function(n){
      n.classList.remove('on');
    });
    var card=document.getElementById('event-'+id);
    var mark=document.querySelector('.ev[data-event="'+CSS.escape(id)+'"]');
    if(card){card.classList.add('on');card.scrollIntoView({block:'center'});}
    if(mark)mark.classList.add('on');
  }
  document.querySelectorAll('.ev').forEach(function(g){
    g.addEventListener('click',function(){selectEvent(g.getAttribute('data-event'));});
    g.addEventListener('keydown',function(e){
      if(e.key==='Enter'||e.key===' '){e.preventDefault();selectEvent(g.getAttribute('data-event'));}
    });
  });
  document.querySelectorAll('.events li').forEach(function(li){
    li.addEventListener('click',function(){selectEvent(li.id.replace('event-',''));});
  });
})();
"#;

/// Everything phase 5 adds to the script.
///
/// **Every view it touches is already drawn.** It filters, it toggles and it
/// composes a tooltip; it renders nothing. With scripting off the page is whole
/// — every person, every topic, every event and every presence row is there,
/// and the only things missing are the search box's effect and the hover
/// numbers, both of which were never anything but script. A report that renders
/// blank without JS has stopped being an archive.
const JS_SURFACE: &str = r#"
(function(){
  var root=document.documentElement;

  /* -- dark only --------------------------------------------------------- */
  /* The script above is frozen -- it is `report.py`'s, character for character,
     and the parity legs compare it -- so it still restores a theme from
     localStorage under a key an older report may well have written. This
     document has no `html.light` block to restore into, and no switch to get
     back from it, so a stored 'light' would leave a page with no colours at
     all. Undo it, and forget the key so it cannot happen twice. */
  root.classList.remove('light');
  root.classList.add('dark');
  try{localStorage.removeItem('tg-report-theme');}catch(e){}

  /* -- presence tooltips ------------------------------------------------- */
  /* The compact rows carry their counts once per row rather than once per
     cell, which is most of the size budget. Every bucket is the same width, so
     the pointer's x IS the bucket index; the label comes from the one shared
     axis list. This also answers over the silent days, where the old per-cell
     tooltip had no element to hover at all. */
  var axisEl=document.getElementById('axis');
  var DAYS=axisEl?(axisEl.getAttribute('data-days')||'').split('|'):[];
  var tip=document.getElementById('tip');
  function counts(svg){
    if(svg._n)return svg._n;
    var out={},raw=svg.getAttribute('data-n')||'';
    raw.split(',').forEach(function(pair){
      if(!pair)return;
      var bits=pair.split(':');
      out[bits[0]]=bits[1];
    });
    svg._n=out;return out;
  }
  document.addEventListener('mousemove',function(e){
    var svg=e.target.closest?e.target.closest('svg.strip'):null;
    if(!svg||!svg.hasAttribute('data-n'))return;
    var box=svg.getBoundingClientRect();
    var total=parseInt(svg.getAttribute('data-b'),10)||1;
    var i=Math.floor((e.clientX-box.left)/box.width*total);
    if(i<0)i=0; if(i>=total)i=total-1;
    var n=counts(svg)[i]||'0';
    var label=DAYS[i]||'';
    tip.textContent=label?label+' · '+n:n;
    tip.classList.add('on');
    tip.style.left=e.clientX+'px';tip.style.top=e.clientY+'px';
  },true);

  /* -- kind filters ------------------------------------------------------ */
  document.querySelectorAll('.kind-filter').forEach(function(b){
    b.addEventListener('click',function(){
      var on=b.getAttribute('aria-pressed')!=='true';
      b.setAttribute('aria-pressed',on?'true':'false');
      root.classList.toggle('hide-k'+b.getAttribute('data-k'),!on);
    });
  });

  /* -- search ------------------------------------------------------------ */
  /* Over the text already on the page, so it cannot disagree with what is
     rendered. Rows are collected once: re-querying the DOM on every keystroke
     is what makes a 250-row table feel broken while you type. */
  var find=document.getElementById('find');
  var found=document.getElementById('found');
  if(find){
    var groups=[];
    document.querySelectorAll('#people tbody tr,#topics tbody tr').forEach(function(n){
      groups.push(n);
    });
    document.querySelectorAll('.events li,.records li').forEach(function(n){
      groups.push(n);
    });
    var hay=groups.map(function(n){return (n.textContent||'').toLowerCase();});
    function run(){
      var q=find.value.trim().toLowerCase();
      root.classList.toggle('searching',q.length>0);
      if(!q){
        groups.forEach(function(n){n.classList.remove('nomatch');});
        found.textContent='';
        return;
      }
      var hits=0;
      for(var i=0;i<groups.length;i++){
        var ok=hay[i].indexOf(q)!==-1;
        groups[i].classList.toggle('nomatch',!ok);
        if(ok)hits++;
      }
      found.textContent=hits+' of '+groups.length;
    }
    find.addEventListener('input',run);
    find.addEventListener('keydown',function(e){
      if(e.key==='Escape'){find.value='';run();}
    });
  }
})();
"#;

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

pub const SOURCE: &str = "Telegram Export Analyser";

/// Today, as the notes section stamps it: `5 September 2025`.
pub fn today_stamp() -> String {
    let text = chrono::Local::now().format("%d %B %Y").to_string();
    text.trim_start_matches('0').to_string()
}

#[derive(Debug, Clone)]
pub struct Options {
    pub embed_fonts: bool,
    /// Who the notes section says wrote the file.
    pub source: String,
    /// The date the notes section stamps.
    ///
    /// Injected rather than read from the clock inside `render`, so a golden
    /// file is stable and the parity diff has nothing to normalise. The Python
    /// calls `datetime.now()` in place; here the caller does it, which is the
    /// only behavioural difference in the port and it is one that makes the
    /// output *more* reproducible, not less.
    pub stamp: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            embed_fonts: true,
            source: SOURCE.to_string(),
            stamp: today_stamp(),
        }
    }
}

/// Peer key -> display name, built from the stats themselves.
///
/// The two charts that label by key — the who-answers-whom matrix and the
/// contact graph — need a name for an arbitrary peer key. `people.rows` carries
/// one for every key that can reach them: anyone who spoke has a row, anyone
/// who only ever reacted gets one through `votes_given`, and a roster member
/// who never posted gets one as `silent`. Building the lookup from the stats
/// rather than taking `tga_metrics::People` is what keeps this crate off
/// `tga-metrics` and lets a recorded `stats.json` render on its own.
pub fn names_from_stats(stats: &Value) -> Names {
    stats
        .get("people")
        .and_then(|folk| folk.get("rows"))
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let key = row.get("key")?.as_str()?;
                    let name = row.get("name")?.as_str()?;
                    Some((key.to_string(), name.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The whole report, as one string.
pub fn render(stats: &Value, names: &Names, notes: &Notes, options: &Options) -> String {
    let title = format!(
        "{} — archive report",
        stats["export"]["name"].as_str().unwrap_or_default()
    );

    let body = format!(
        "{}{}{}{}{}{}{}{}{}</section>{}{}{}",
        sections::masthead(stats),
        sections::timeline(stats, notes, names),
        sections::rhythm(stats),
        sections::people(stats),
        sections::conversation(stats, names),
        sections::between(stats),
        sections::said(stats),
        // The churn section's heading is written here rather than inside
        // `churn`, because that function returns nothing at all for an export
        // with no dated months and the `<section>` still has to close.
        section_head("Coming and going", "churn"),
        sections::churn(stats),
        sections::topics(stats),
        sections::records(stats),
        sections::notes(stats, &notes.source, &options.source, &options.stamp),
    );

    // The compact presence rows leave their bucket labels out of every cell —
    // that omission is most of the size budget — so the labels are written into
    // the document once, here, and the tooltip is composed from the pointer's
    // position.
    let axis = shared_axis(stats);
    let extra_css = CSS_SURFACE;
    let extra_js = JS_SURFACE;
    let kind_rules = kind_css(notes);

    format!(
        "<!doctype html>\n<html lang=\"en\" class=\"{}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{}</title><style>{}{}{CSS}{extra_css}{kind_rules}</style></head><body>{body}{axis}\
         <div id=\"tip\" role=\"status\"></div><script>{JS}{extra_js}</script></body></html>\n",
        palette::DEFAULT,
        esc(&title),
        assets::font_css(options.embed_fonts),
        tokens_css(),
    )
}

/// Every presence row's bucket labels, written once for the document.
///
/// Every row is bucketed identically — same date range, same width, same
/// `bucket_days` — so one list serves all 260 of them. Emitting it per cell is
/// what made the presence rows 64% of a large report.
fn shared_axis(stats: &Value) -> String {
    let series = stats::pairs(&stats["activity"], "per_day");
    if series.is_empty() {
        return String::new();
    }
    let labels = charts::bucket_labels(&series, sections::PRESENCE_WIDTH);
    format!(
        "<div id=\"axis\" hidden data-days=\"{}\"></div>",
        esc(&labels.join("|"))
    )
}

/// The show/hide rule for each kind the notes file actually used.
///
/// Generated rather than fixed at four, because the vocabulary belongs to
/// whoever wrote the file. A file with three kinds gets three rules and no dead
/// fourth; a file with six gets six.
fn kind_css(notes: &Notes) -> String {
    let kinds = notes.kinds();
    if kinds.len() < 2 {
        return String::new();
    }
    (0..kinds.len())
        .map(|k| format!("html.hide-k{k} .ev.k{k},html.hide-k{k} .events li.k{k}{{display:none}}"))
        .collect()
}

fn section_head(title: &str, anchor: &str) -> String {
    format!(
        "<section id=\"{}\"><div class=\"sechead\"><h2>{}</h2><span class=\"count\"></span></div>",
        esc(anchor),
        esc(title)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The smallest stats value that exercises every branch's absence.
    fn bare() -> Value {
        json!({
            "export": { "name": "Nothing", "root": "C:\\x", "topics": 0, "messages": 0 },
            "activity": { "empty": true },
            "people": { "rows": [], "speakers": 0, "known_members": 0,
                        "roster_complete": null, "silent_members": 0,
                        "top3_share": 0.0, "votes_named": 0, "votes_total": 0 },
            "content": { "messages": 0, "lengths": [], "media_kinds": [] },
            "conversation": { "replies": 0, "sessions": 0, "session_gap": 1800,
                              "self_replies": 0, "fastest": [], "edges": [],
                              "starters": [], "orphan_replies": 0 },
            "graph": { "nodes": [], "edges": [], "hidden": 0 },
            "topics": [], "superlatives": [], "awards": [],
            "streak": {}, "churn": { "months": [] }, "renamed": []
        })
    }

    /// Five days of activity, so the timeline has an axis to draw on.
    fn dated() -> Value {
        json!({
            "empty": false, "first": "2025-01-01", "last": "2025-01-05",
            "span_days": 5, "active_days": 2,
            "per_day": [["2025-01-01", 3], ["2025-01-02", 0], ["2025-01-03", 1],
                        ["2025-01-04", 0], ["2025-01-05", 2]],
            "per_month": [["2025-01", 6]],
            "per_hour": vec![0; 24], "per_weekday": vec![0; 7],
            "hour_weekday": vec![vec![0; 24]; 7],
            "per_day_by_topic": {}, "per_day_by_person": {}
        })
    }

    fn options() -> Options {
        Options {
            stamp: "5 September 2025".into(),
            ..Default::default()
        }
    }

    #[test]
    fn an_export_with_nothing_in_it_still_renders_a_whole_document() {
        // The report is what somebody gets back from a folder that turned out
        // to be empty. Failing here would leave them with no output and no
        // explanation.
        let html = render(&bare(), &Names::new(), &Notes::default(), &options());
        assert!(html.starts_with("<!doctype html>\n"));
        assert!(html.ends_with("</body></html>\n"));
        assert!(html.contains("No dated messages."));
        assert!(html.contains("no dated messages"));
    }

    #[test]
    fn every_section_closes_the_tag_it_opened() {
        let html = render(&bare(), &Names::new(), &Notes::default(), &options());
        assert_eq!(
            html.matches("<section").count(),
            html.matches("</section>").count(),
            "unbalanced sections"
        );
    }

    #[test]
    fn the_report_is_dark_and_offers_no_choice_about_it() {
        let html = render(&bare(), &Names::new(), &Notes::default(), &options());
        assert!(html.contains("<html lang=\"en\" class=\"dark\">"));
        assert!(html.contains("html.dark{"));
        assert!(html.contains("--r5:"));
        assert!(html.contains("--self:"));

        // One block, one switch fewer. A light block nothing can reach is dead
        // weight in every report ever written; a switch to it with no block is
        // a page with no colours.
        assert!(!html.contains("html.light{"), "the surface emits one block");
        assert!(
            !html.contains("id=\"theme\""),
            "and no control to change it"
        );
    }

    /// A `dynamics` branch whose people have stopped posting.
    ///
    /// The golden fixture cannot reach this: its archive is seven days long, so
    /// nobody in it can be thirty days dormant and the section correctly renders
    /// the "everybody posted in the last month" line instead. The populated
    /// table is real markup and needs a case of its own.
    fn drifted() -> Value {
        let mut stats = bare();
        stats["dynamics"] = json!({
            "empty": false,
            "pairs": { "rows": [], "shown": 0, "mutual": 0, "one_way": 0, "directed": 0 },
            "answer": {
                "counts": vec![0; 24], "medians": vec![0; 24],
                "counted": 0, "minimum": 20, "cap": 86400,
                "fastest_hour": Value::Null, "slowest_hour": Value::Null,
            },
            "tenure": {
                "as_of": "2025-06-30", "active": 0, "fading": 1, "gone": 1,
                "active_within": 30, "fading_within": 90,
                "rows": [
                    { "key": "user1", "name": "Ana", "messages": 400,
                      "first": "2025-01-01", "last": "2025-05-20", "span_days": 140,
                      "active_days": 40, "density": 0.2857142857142857,
                      "dormant_days": 41, "status": "fading" },
                    { "key": "user2", "name": "Bob & Co <the second>", "messages": 12,
                      "first": "2025-01-01", "last": "2025-01-04", "span_days": 4,
                      "active_days": 2, "density": 0.5,
                      "dormant_days": 177, "status": "gone" },
                ],
            },
            "retention": {
                "months": [], "active": [], "new": [], "returning": [], "lost": [],
                "people": 2, "kept_mean": 0.0, "months_counted": 0,
            },
            "depth": {
                "buckets": [], "cap": 8, "chained": 0, "max": 0,
                "median": 0, "mean": 0.0, "longest": Value::Null,
            },
        });
        stats
    }

    #[test]
    fn the_people_who_stopped_posting_are_ranked_by_the_silence() {
        let html = render(&drifted(), &Names::new(), &Notes::default(), &options());
        // Longest silence first, which is the one ordering the People table
        // cannot give — it ranks by message count, and there Ana comes first.
        let bob = html.find("Bob &amp; Co").expect("the quiet one is listed");
        let ana = html.find(">Ana<").expect("the fading one is listed");
        assert!(
            bob < ana,
            "the table is ranked by dormancy, not by messages"
        );
        assert!(html.contains("<td class=\"n\">177</td>"));
        assert!(html.contains("30 Jun 2025, the last day in this archive"));
        // Escaped like every other name that came out of an export.
        assert!(!html.contains("Bob & Co <the second>"));
    }

    #[test]
    fn a_report_rendered_from_a_dump_without_dynamics_is_still_whole() {
        // Every `--from-stats` dump recorded before the branch existed has this
        // shape, and a re-render of one must not lose a section or a nav entry
        // it never had.
        let html = render(&bare(), &Names::new(), &Notes::default(), &options());
        assert!(!html.contains("id=\"between\""));
        assert!(!html.contains("#between"));
        assert!(html.ends_with("</body></html>\n"));
    }

    #[test]
    fn a_stored_light_theme_cannot_leave_the_surface_with_no_colours() {
        // The frozen script restores `tg-report-theme` from localStorage, and
        // an older report may well have written 'light' under that key. With no
        // `html.light` block and no switch, restoring it would render a page
        // with no colours and no way back. The surface script undoes it.
        let html = render(&bare(), &Names::new(), &Notes::default(), &options());
        assert!(html.contains("root.classList.remove('light')"));
        assert!(html.contains("localStorage.removeItem('tg-report-theme')"));
    }

    #[test]
    fn a_hostile_export_name_cannot_escape_into_markup() {
        // A report opens as a local file, so anything that survives as markup
        // runs with that origin. The name came from a group title.
        let mut stats = bare();
        stats["export"]["name"] = json!("<script>alert(1)</script>");
        let html = render(&stats, &Names::new(), &Notes::default(), &options());
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    #[test]
    fn a_hostile_event_title_cannot_escape_either() {
        // The events file is written outside the program entirely, so it is
        // the least trusted input on the page.
        let event = tga_notes::Event {
            id: "\"><img src=x onerror=alert(1)>".into(),
            start: chrono::NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            end: None,
            title: "<b>bold</b>".into(),
            summary: "<i>x</i>".into(),
            kind: "milestone".into(),
            topic: None,
            messages: vec![1],
            confidence: String::new(),
            time: String::new(),
            weight: String::new(),
            who: vec![],
            tags: vec![],
        };
        let mut stats = bare();
        stats["activity"] = dated();
        let notes = Notes {
            events: vec![event],
            source: "events.json".into(),
            ..Default::default()
        };
        let html = render(&stats, &Names::new(), &notes, &options());
        assert!(!html.contains("<img src=x"));
        assert!(!html.contains("<b>bold</b>"));
        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"));
        assert!(html.contains("Events read from events.json."));
    }

    #[test]
    fn no_events_file_says_how_to_add_one_rather_than_saying_nothing() {
        let mut stats = bare();
        stats["activity"] = dated();
        let html = render(&stats, &Names::new(), &Notes::default(), &options());
        assert!(html.contains("No events file yet."));
        assert!(html.contains("analysis/digest.jsonl"));
    }

    #[test]
    fn an_export_with_dates_draws_the_hero_ribbon_and_its_axis() {
        let mut stats = bare();
        stats["activity"] = dated();
        let html = render(&stats, &Names::new(), &Notes::default(), &options());
        assert!(html.contains("class=\"chart ribbon hero\""));
        assert!(html.contains("class=\"chart axis-strip\""));
        assert!(html.contains("class=\"chart calendar\""));
        // The caption states the grain, which is the whole point of bucketing.
        assert!(html.contains("One bar per day; the tallest is 3 messages."));
    }

    #[test]
    fn names_are_taken_from_the_people_rows() {
        let stats = json!({
            "people": { "rows": [
                { "key": "user1", "name": "Ana" },
                { "key": "user2", "name": "Bob" },
            ]}
        });
        let names = names_from_stats(&stats);
        assert_eq!(names.get("user1").map(String::as_str), Some("Ana"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn no_fonts_leaves_the_document_without_a_single_font_face() {
        let html = render(
            &bare(),
            &Names::new(),
            &Notes::default(),
            &Options {
                embed_fonts: false,
                ..options()
            },
        );
        assert!(!html.contains("@font-face"));
        // ...and still names Geist in the stack, so an installed copy is used.
        assert!(html.contains("font-family:'Geist'"));
    }

    #[test]
    fn the_report_asks_nothing_of_the_network() {
        // The one property that makes this an archive rather than a page.
        let html = render(&bare(), &Names::new(), &Notes::default(), &options());
        for scheme in ["http://", "https://", "//cdn", "src=\"http"] {
            assert!(!html.contains(scheme), "the report reaches for {scheme}");
        }
    }

    #[test]
    fn the_broken_marker_is_still_overridden_rather_than_merely_absent() {
        // Two rules, and the reader only ever sees the second. The first is the
        // inherited U+0091 defect and the second corrects it, so both have to
        // be here in this order or an open disclosure shows a stray "2". When
        // the stylesheet moves out into its own file the two collapse into one
        // and this test loses its first assertion.
        let html = render(&bare(), &Names::new(), &Notes::default(), &options());
        let broken = html
            .find("details.data[open] summary::before{content:'\u{91}2 '}")
            .expect("the inherited defect");
        let fixed = html
            .find("details.data[open] summary::before{content:'\\2212 '}")
            .expect("and the rule that beats it");
        assert!(broken < fixed, "the correction has to come last to win");
    }
}
