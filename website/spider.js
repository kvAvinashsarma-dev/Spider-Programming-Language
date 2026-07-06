/* Spider in the browser — loads the real compiler+VM (spider.wasm) and
   wires every .embed block and the big playground. No frameworks. */

const SpiderEngine = (() => {
  let instance = null;
  let loading = null;
  const enc = new TextEncoder();
  const dec = new TextDecoder();

  async function load() {
    if (instance) return instance;
    if (!loading) {
      loading = (async () => {
        const resp = await fetch("spider.wasm");
        const bytes = await resp.arrayBuffer();
        const { instance: inst } = await WebAssembly.instantiate(bytes, {});
        instance = inst;
        return inst;
      })();
    }
    return loading;
  }

  function writeStr(inst, str) {
    const bytes = enc.encode(str);
    const ptr = inst.exports.sp_alloc(bytes.length);
    new Uint8Array(inst.exports.memory.buffer, ptr, bytes.length).set(bytes);
    return [ptr, bytes.length];
  }

  async function run(src, input) {
    const inst = await load();
    const [sp, sl] = writeStr(inst, src);
    const [ip, il] = writeStr(inst, input || "");
    const len = inst.exports.sp_run(sp, sl, ip, il);
    const raw = dec.decode(
      new Uint8Array(inst.exports.memory.buffer, inst.exports.sp_out_ptr(), len)
    );
    const nl = raw.indexOf("\n");
    const status = nl === -1 ? raw : raw.slice(0, nl);
    const text = nl === -1 ? "" : raw.slice(nl + 1);
    return { status, text };
  }

  return { run, load };
})();

function paintOutput(box, result) {
  box.classList.remove("ok", "err");
  if (result.status === "OK") {
    box.classList.add("ok");
    box.textContent = result.text.length ? result.text : "(the program printed nothing)";
  } else {
    box.classList.add("err");
    box.textContent = result.text;
  }
}

async function runInto(srcEl, outBox, inputEl) {
  outBox.textContent = "running…";
  outBox.classList.remove("ok", "err");
  try {
    const result = await SpiderEngine.run(srcEl.value, inputEl ? inputEl.value : "");
    paintOutput(outBox, result);
  } catch (e) {
    outBox.classList.add("err");
    outBox.textContent = "the engine could not load: " + e;
  }
}

/* Lesson embeds: <div class="embed" data-code="..."> */
document.addEventListener("DOMContentLoaded", () => {
  document.querySelectorAll(".embed").forEach((el) => {
    const code = el.dataset.code ? decodeURIComponent(el.dataset.code) : "";
    const needsInput = el.dataset.input !== undefined;
    const ta = document.createElement("textarea");
    ta.className = "editor";
    ta.spellcheck = false;
    ta.value = code;
    const row = document.createElement("div");
    row.className = "runrow";
    const btn = document.createElement("button");
    btn.className = "btn";
    btn.textContent = "▶ Run";
    row.appendChild(btn);
    let inputEl = null;
    if (needsInput) {
      inputEl = document.createElement("input");
      inputEl.placeholder = "answers for ask, one per line…";
      inputEl.value = el.dataset.input || "";
      inputEl.style.flex = "1";
      inputEl.style.background = "var(--card)";
      inputEl.style.color = "var(--text)";
      inputEl.style.border = "1px solid var(--border)";
      inputEl.style.borderRadius = "8px";
      inputEl.style.padding = "9px 12px";
      row.appendChild(inputEl);
    }
    const hint = document.createElement("span");
    hint.className = "hint";
    hint.textContent = "edit the code — it runs the real compiler";
    row.appendChild(hint);
    const out = document.createElement("div");
    out.className = "outbox";
    out.textContent = "output appears here";
    el.append(ta, row, out);
    const go = () => runInto(ta, out, inputEl);
    btn.addEventListener("click", go);
    ta.addEventListener("keydown", (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") go();
      if (e.key === "Tab") {
        e.preventDefault();
        const s = ta.selectionStart;
        ta.value = ta.value.slice(0, s) + "    " + ta.value.slice(ta.selectionEnd);
        ta.selectionStart = ta.selectionEnd = s + 4;
      }
    });
  });

  /* The big playground page */
  const src = document.getElementById("pg-src");
  if (src) {
    const out = document.getElementById("pg-out");
    const inp = document.getElementById("pg-input");
    const btn = document.getElementById("pg-run");
    const sel = document.getElementById("pg-examples");
    const go = () => runInto(src, out, inp);
    btn.addEventListener("click", go);
    src.addEventListener("keydown", (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") go();
      if (e.key === "Tab") {
        e.preventDefault();
        const s = src.selectionStart;
        src.value = src.value.slice(0, s) + "    " + src.value.slice(src.selectionEnd);
        src.selectionStart = src.selectionEnd = s + 4;
      }
    });
    if (sel) {
      sel.addEventListener("change", () => {
        const ex = PG_EXAMPLES[sel.value];
        if (ex) {
          src.value = ex.code;
          inp.value = ex.input || "";
          out.textContent = "output appears here";
          out.classList.remove("ok", "err");
        }
      });
    }
    SpiderEngine.load().then(() => {
      document.getElementById("pg-status").textContent =
        "engine loaded — the real Spider compiler is running in your browser";
      go();
    });
  }
});

const PG_EXAMPLES = {
  hello: {
    code: 'say "Hello, world!"\n\nlet name = ask "What is your name?"\nsay "Welcome to Spider, {name}!"\n',
    input: "Ada",
  },
  shapes: {
    code:
      'record Point\n    x: Float\n    y: Float\n\nchoice Shape\n    Circle(radius: Float)\n    Rect(width: Float, height: Float)\n    Dot\n\nfn area(shape: Shape) -> Float\n    match shape\n        Circle(r) -> 3.14159 * r * r\n        Rect(w, h) -> w * h\n        Dot -> 0.0\n\nfn main()\n    for shape in [Circle(1.0), Rect(2.0, 3.0), Dot]\n        say "area: {area(shape)}"\n    say Point(3.0, 4.0)\n',
  },
  outcomes: {
    code:
      'fn parse_grade(score: Int) -> Outcome of Text\n    if score < 0\n        return Fail("scores start at zero")\n    if score >= 90\n        return Ok("A")\n    return Ok("keep going")\n\nfor score in [95, 40, -3]\n    match parse_grade(score)\n        Ok(grade) -> say "{score}: {grade}"\n        Fail(problem) -> say "{score}: oops — {problem}"\n',
  },
  loops: {
    code:
      'var total = 0\nfor i in 1 to 10\n    total += i\nsay "1 to 10 adds up to {total}"\n\nrepeat 3 times\n    say "hip hip hooray!"\n\nfn fib(n: Int) -> Int\n    if n < 2\n        return n\n    return fib(n - 1) + fib(n - 2)\n\nsay "fib(15) = {fib(15)}"\n',
  },
  teacher: {
    code:
      '# Spider\'s compiler is a teacher. Run this and read the message —\n# it says what happened, why, and how to fix it. Then fix it!\nlet total = 5\nsay totl\n',
  },
  safety: {
    code:
      '# Safe Mode: browser programs get zero capabilities.\n# Spider refuses this politely — with the fix spelled out.\nuse files\nsay files.exists("secrets.txt")\n',
  },
};
