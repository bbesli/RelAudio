<script lang="ts">
  import Icon from "./Icon.svelte";

  /**
   * İki makineyi bir kez eşleştiren kart.
   *
   * **Kod her iki makinede de görünüyor.** İlk tasarımda kod yalnızca
   * "başlat"a basılan makinede çıkıyordu; kullanıcı kodu girmek istediği
   * makineye gittiğinde ortada okuyacağı bir kod yoktu ve akış orada
   * tıkanıyordu. Artık kart açıkken bu makine kendi kodunu gösteriyor ve
   * karşının kodunu kabul ediyor — hangi yönden girildiği fark etmiyor.
   */
  let {
    peerName,
    code,
    seconds,
    busy,
    onsubmit,
    t,
  }: {
    peerName: string;
    code: string | null;
    seconds: number;
    busy: boolean;
    onsubmit: (code: string) => void;
    t: (key: string, vars?: Record<string, string | number>) => string;
  } = $props();

  let input = $state("");

  /** Yalnızca rakam, en fazla 6. Yapıştırılan "418 205" gibi biçimler de tutsun. */
  function onInput(e: Event & { currentTarget: HTMLInputElement }) {
    input = e.currentTarget.value.replace(/\D/g, "").slice(0, 6);
    e.currentTarget.value = input;
  }

  function submit() {
    if (input.length === 6 && !busy) onsubmit(input);
  }

  const mmss = $derived(
    `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`
  );
</script>

<section class="pair" aria-labelledby="pair-title">
  <header>
    <span class="badge"><Icon name="link" size={16} /></span>
    <div>
      <h3 id="pair-title">{t("pair.enterTitle", { device: peerName })}</h3>
      <p>{t("pair.showBody", { seconds: String(seconds) })}</p>
    </div>
  </header>

  <div class="halves">
    <div class="half own">
      <span class="label">{t("pair.yourCode")}</span>
      {#if code}
        <div class="code" aria-live="polite">
          <span>{code.slice(0, 3)}</span><span>{code.slice(3)}</span>
        </div>
        <span class="meta">{t("pair.showTitle", { device: peerName })} · {mmss}</span>
      {:else}
        <div class="code placeholder">— — —</div>
      {/if}
    </div>

    <div class="or" aria-hidden="true"></div>

    <div class="half theirs">
      <label class="label" for="pair-input">{t("pair.enterBody", { device: peerName })}</label>
      <div class="entry">
        <input id="pair-input" type="text" inputmode="numeric" autocomplete="one-time-code"
               maxlength="7" placeholder="000000" value={input}
               oninput={onInput}
               onkeydown={(e) => { if (e.key === "Enter") submit(); }} />
        <button class="primary" disabled={busy || input.length !== 6} onclick={submit}>
          {t("pair.submit")}
        </button>
      </div>
    </div>
  </div>
</section>

<style>
  .pair {
    border: 1px solid var(--accent-line);
    background: var(--accent-soft);
    border-radius: var(--r-lg);
    padding: 18px;
    margin-bottom: 18px;
  }
  header { display: flex; gap: 12px; align-items: flex-start; margin-bottom: 16px; }
  .badge {
    flex: none; display: grid; place-items: center;
    width: 32px; height: 32px; border-radius: 10px;
    background: var(--accent); color: var(--on-accent);
  }
  h3 { margin: 0; font-size: 15px; font-weight: 650; }
  header p { margin: 3px 0 0; color: var(--dim); font-size: 12.5px; line-height: 1.5; }

  .halves { display: flex; flex-wrap: wrap; gap: 14px; align-items: stretch; }
  .half {
    flex: 1 1 210px; min-width: 0;
    background: var(--surface); border: 1px solid var(--line);
    border-radius: var(--r-md); padding: 14px;
    display: flex; flex-direction: column; gap: 8px;
  }
  .or { display: none; }
  .label { font-size: 12px; color: var(--dim); font-weight: 500; }
  .code {
    display: flex; gap: 10px;
    font-size: 30px; font-weight: 700; letter-spacing: 3px;
    font-variant-numeric: tabular-nums; color: var(--text);
    user-select: all;
  }
  .code.placeholder { color: var(--dim); letter-spacing: 6px; }
  .meta { font-size: 11.5px; color: var(--dim); }

  .entry { display: flex; gap: 8px; margin-top: auto; }
  .entry input {
    flex: 1; min-width: 0;
    font-size: 20px; letter-spacing: 5px; text-align: center;
    font-variant-numeric: tabular-nums; padding: 9px 10px;
  }
  .entry button { flex: none; }
</style>
