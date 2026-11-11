import { useCallback, useEffect, useRef, useState } from 'react'
import { EasterEgg } from '@/main/EasterEgg'
import { useStore } from '@/main/store'
import { track } from '@/shared/analytics'
import { useI18n } from '@/shared/i18n'
import { formatCmdError, type ModelsList, modelsList, openContact, openRepo, openSponsor, openUrl } from '@/shared/ipc'
import { DOWNLOAD_URL, SITE_URL } from '@/shared/links'
import { modelTitle } from '@/shared/models'
import { ModelKind } from '@/shared/types'
import { Button } from '@/ui/Button'
import { Icon } from '@/ui/Icon'
import { Page } from '@/ui/Page'
import { SettingsGroup, SettingsRow } from '@/ui/Settings'
import thirdPartyNotices from '../../../docs/THIRD_PARTY_NOTICES.md?raw'

const EGG_TAPS = 7
const EGG_TAP_WINDOW = 1100

const VERSION_COLORS = ['#fc413d', '#ff6b2b', '#fec700', '#88de42', '#00a9bb', '#3186ff', '#9378ff'] as const

export function AboutPage() {
  const { info, setError } = useStore()
  const { t } = useI18n()
  const [models, setModels] = useState<ModelsList | null>(null)
  const [modelsFailed, setModelsFailed] = useState(false)
  const [licensesOpen, setLicensesOpen] = useState(false)
  const [eggOpen, setEggOpen] = useState(false)
  const [eggTaps, setEggTaps] = useState(0)
  const [versionPulse, setVersionPulse] = useState(0)
  const lastEggTap = useRef(0)
  const closeEgg = useCallback(() => setEggOpen(false), [])
  const run = useCallback(
    (action: () => Promise<unknown>) => {
      void action().catch((e) => setError(formatCmdError(e)))
    },
    [setError],
  )

  function tapVersion() {
    setVersionPulse((n) => n + 1)
    const now = performance.now()
    const taps = now - lastEggTap.current > EGG_TAP_WINDOW ? 1 : eggTaps + 1
    lastEggTap.current = now
    if (taps >= EGG_TAPS) {
      setEggTaps(0)
      track('easter_egg_opened', { version: info?.version ?? 'unknown' })
      setEggOpen(true)
      return
    }
    setEggTaps(taps)
  }

  useEffect(() => {
    let disposed = false
    modelsList()
      .then((m) => {
        if (!disposed) setModels(m)
      })
      .catch(() => {
        if (!disposed) setModelsFailed(true)
      })
    return () => {
      disposed = true
    }
  }, [])

  return (
    <Page title={t('about.title')}>
      <SettingsGroup title={t('about.appGroup')}>
        <SettingsRow
          icon="info"
          title={t('about.version')}
          onClick={tapVersion}
          noHover
          control={
            <Mono>
              <span
                key={versionPulse}
                className={versionPulse > 0 ? 'inline-block animate-[egg-tap_260ms_ease-out]' : 'inline-block'}
                style={versionPulse > 0 ? { color: VERSION_COLORS[versionPulse % VERSION_COLORS.length] } : undefined}
              >
                {info?.version ?? '—'}
              </span>
            </Mono>
          }
        />
        <SettingsRow
          icon="download"
          title={t('about.manualUpdate')}
          description={t('about.manualDistribution')}
          control={
            <Button size="sm" icon="external" onClick={() => run(() => openUrl(DOWNLOAD_URL))}>
              {t('about.manualUpdate')}
            </Button>
          }
        />
        <SettingsRow
          icon="heart"
          title={t('about.upstreamCreditTitle')}
          description={t('about.upstreamCreditBody')}
          onClick={() => run(() => openUrl('https://github.com/niyongsheng/AudioScript'))}
          control={<Icon name="external" size={16} />}
        />
        <SettingsRow
          icon="chip"
          title={t('about.platform')}
          control={
            <Mono>
              {info?.platform ?? '—'} · {info?.arch ?? '—'}
            </Mono>
          }
        />
        <SettingsRow
          icon="external"
          title={t('about.website')}
          onClick={() => run(() => openUrl(SITE_URL))}
          control={<Icon name="chevron" size={16} className="mt-0.5 text-[var(--color-ink-2)] rtl:rotate-180" />}
        />
      </SettingsGroup>

      <SettingsGroup title={t('about.support')}>
        <SettingsRow
          icon="heart"
          title={t('about.sponsor')}
          description={t('about.sponsorDescription')}
          onClick={() => run(openSponsor)}
          control={<Icon name="external" size={16} />}
        />
      </SettingsGroup>

      <SettingsGroup title={t('about.licensesGroup')}>
        <SettingsRow
          icon="github"
          title={
            <span className="flex items-center gap-2">
              LICENSE
              <Mono>MIT</Mono>
            </span>
          }
          onClick={() => void openRepo()}
          control={<Icon name="chevron" size={16} className="mt-0.5 text-[var(--color-ink-2)] rtl:rotate-180" />}
        />
      </SettingsGroup>

      <SettingsGroup title={t('about.modelsLicenses')} footer={licensesOpen ? t('about.licenseFooter') : undefined}>
        <SettingsRow
          icon="shield"
          title={t('about.modelLicense')}
          onClick={() => setLicensesOpen((open) => !open)}
          control={
            <Icon
              name="chevron"
              size={16}
              className={`mt-0.5 text-[var(--color-ink-2)] transition-transform ${licensesOpen ? 'rotate-90' : ''}`}
            />
          }
        />
        {licensesOpen &&
          (models ? (
            [...models.asr, ...models.mt].flatMap((m) =>
              m.licenses.map((l) => (
                <SettingsRow
                  key={`${m.id}-${l.component}`}
                  icon={m.kind === ModelKind.Asr ? 'waveform' : 'translate'}
                  title={l.component}
                  description={
                    <>
                      {modelTitle(m, t)} · {l.license}
                      {l.requiresAttribution && t('about.needsAttribution')}
                      {l.attribution && ` · ${l.attribution}`}
                    </>
                  }
                  control={
                    l.licenseUrl ? (
                      <Button
                        size="sm"
                        variant="ghost"
                        icon="external"
                        onClick={() => run(() => openUrl(l.licenseUrl as string))}
                      >
                        {t('about.licenseButton')}
                      </Button>
                    ) : undefined
                  }
                />
              )),
            )
          ) : (
            <SettingsRow title={modelsFailed ? t('about.licenseLoadFailed') : t('common.loading')} />
          ))}
      </SettingsGroup>

      {licensesOpen && (
        <pre className="whitespace-pre-wrap break-words rounded-xl bg-[var(--color-surface-1)] p-4 text-xs leading-relaxed">
          {thirdPartyNotices}
        </pre>
      )}

      {eggOpen && info && (
        <EasterEgg
          name={info.name}
          version={info.version}
          platform={info.platform}
          arch={info.arch}
          onClose={closeEgg}
          onContact={() =>
            run(async () => {
              await openContact()
              closeEgg()
            })
          }
        />
      )}
    </Page>
  )
}

function Mono({ children }: { children: React.ReactNode }) {
  return <span className="font-mono text-[var(--color-ink-1)] text-xs">{children}</span>
}
