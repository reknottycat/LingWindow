import { useEffect, useState } from 'react'
import { useModels } from '@/main/useModels'
import { useI18n } from '@/shared/i18n'
import { EVENT, EngineType, TranslateEngine, type Preferences, prefsGet, sessionSetDirection, formatCmdError, macTranslationLanguages } from '@/shared/ipc'
import { APPLE_TRANSLATION_FALLBACKS, appleTargetChoices, displayNameFormatter, multilingualChoices, sourceLanguagesFor, targetLanguagesFor, languageIdFor, languageLabel, isSupportedPair, scriptForPair, preferredTarget, type LanguageChoice } from '@/shared/languages'
import { useBroadcastSnapshot } from '@/shared/useBroadcastSnapshot'

export function DirectionControls({ onError }: { onError: (message: string) => void }) {
  const { locale, t } = useI18n()
  const { list } = useModels()
  const [prefs, setPrefs] = useState<Preferences | null>(null)
  const [busy, setBusy] = useState(false)
  const [apple, setApple] = useState<string[]>([...APPLE_TRANSLATION_FALLBACKS])
  useBroadcastSnapshot<Preferences>(EVENT.direction, prefsGet, setPrefs)
  useEffect(() => {
    if (prefs?.translateEngine === TranslateEngine.MacTranslation) void macTranslationLanguages().then(setApple).catch(() => {})
  }, [prefs?.translateEngine])
  if (!prefs || !list) return null
  const mt = list.mt.find((m) => m.selected)
  const asr = list.asr.find((m) => m.selected)
  const display = displayNameFormatter(locale)
  const all = mt?.multilingual ? multilingualChoices(mt.languages, display) : []
  const appleChoices = appleTargetChoices(apple, display)
  const filter = (choices: readonly LanguageChoice[]) => asr?.multilingual && prefs.asrEngine !== EngineType.MacNative ? choices.filter((l) => asr.languages.includes(l.backend)) : choices
  const sources = sourceLanguagesFor(prefs.translateEngine, !!asr?.multilingual || prefs.asrEngine === EngineType.MacNative, filter(appleChoices), filter(all))
  const targets = targetLanguagesFor(prefs.translateEngine, appleChoices, all)
  const source = sources.find((l) => l.id === languageIdFor(prefs.sourceLang, prefs.zhScript, sources))
  const target = targets.find((l) => l.id === languageIdFor(prefs.targetLang, prefs.zhScript, targets))
  if (!source || !target || prefs.translateEngine === TranslateEngine.Off) return null
  const valid = (a: LanguageChoice, b: LanguageChoice) => isSupportedPair(a, b, prefs.translateEngine, !!mt?.multilingual)
  async function apply(a: LanguageChoice, b: LanguageChoice) {
    if (busy || !prefs) return
    setBusy(true)
    try { setPrefs(await sessionSetDirection(a.backend, b.backend, scriptForPair(a, b, prefs.zhScript))) }
    catch (e) { onError(formatCmdError(e)) }
    finally { setBusy(false) }
  }
  const reverseSource = sources.find((l) => l.id === target.id)
  const reverseTarget = targets.find((l) => l.id === source.id)
  const style = 'h-7 min-w-0 max-w-24 rounded bg-[#20202a] px-1 text-[11px] text-white disabled:opacity-40'
  return <div className="flex min-w-0 flex-1 items-center gap-0.5" aria-busy={busy}>
    <select className={style} aria-label={t('translate.sourceLanguage')} title={t('translate.sourceLanguage')} disabled={busy} value={source.id} onChange={(e) => {
      const a = sources.find((l) => l.id === e.target.value)
      if (!a) return
      const b = valid(a, target) ? target : preferredTarget(a, prefs.zhScript, prefs.translateEngine, targets, !!mt?.multilingual)
      if (b) void apply(a, b)
    }}>{sources.map((l) => <option key={l.id} value={l.id}>{languageLabel(l, t)}</option>)}</select>
    <button className="h-7 shrink-0 px-1 text-white disabled:opacity-40" aria-label={t('translate.swapDirection')} title={t('translate.swapDirection')} disabled={busy || !reverseSource || !reverseTarget || !valid(reverseSource, reverseTarget)} onClick={() => { if (reverseSource && reverseTarget) void apply(reverseSource, reverseTarget) }}>⇄</button>
    <select className={style} aria-label={t('translate.targetLanguage')} title={t('translate.targetLanguage')} disabled={busy} value={target.id} onChange={(e) => { const b = targets.find((l) => l.id === e.target.value); if (b) void apply(source, b) }}>{targets.map((l) => <option key={l.id} value={l.id} disabled={!valid(source, l)}>{languageLabel(l, t)}</option>)}</select>
  </div>
}
