import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { CheckCircle, ChevronRight, ChevronLeft, Tv, Film, Loader2 } from 'lucide-react'
import { useSetupInit } from '../hooks/useApi'
import type { SetupInit } from '../api/types'

type DownloadClientType = SetupInit['downloadClientType']

const steps = ['Welcome', 'Download Client', 'Root Folders', 'Complete'] as const

export default function FirstBoot() {
  const navigate = useNavigate()
  const setupMutation = useSetupInit()
  const [step, setStep] = useState(0)

  const [enableTv, setEnableTv] = useState(true)
  const [enableMovies, setEnableMovies] = useState(true)
  const [downloadClientType, setDownloadClientType] = useState<DownloadClientType>('none')
  const [tvRootFolder, setTvRootFolder] = useState('/media/tv')
  const [movieRootFolder, setMovieRootFolder] = useState('/media/movies')
  const [done, setDone] = useState(false)

  const canNext = () => {
    if (step === 0) return enableTv || enableMovies
    return true
  }

  const handleFinish = () => {
    setupMutation.mutate(
      {
        enableTv,
        enableMovies,
        downloadClientType,
        tvRootFolder: enableTv ? tvRootFolder : undefined,
        movieRootFolder: enableMovies ? movieRootFolder : undefined,
      },
      {
        onSuccess: () => setDone(true),
      },
    )
  }

  const handleGoToApp = () => {
    navigate('/series')
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-slate-900 p-4">
      <div className="w-full max-w-lg">
        {/* Step indicator */}
        <div className="mb-8 flex items-center justify-center gap-2">
          {steps.map((label, i) => (
            <div key={label} className="flex items-center gap-2">
              <div
                className={`flex h-8 w-8 items-center justify-center rounded-full text-sm font-medium ${
                  i < step
                    ? 'bg-blue-600 text-white'
                    : i === step
                      ? 'bg-blue-500 text-white ring-2 ring-blue-400 ring-offset-2 ring-offset-slate-900'
                      : 'bg-slate-700 text-slate-400'
                }`}
              >
                {i < step ? <CheckCircle size={16} /> : i + 1}
              </div>
              {i < steps.length - 1 && (
                <div className={`h-0.5 w-8 ${i < step ? 'bg-blue-600' : 'bg-slate-700'}`} />
              )}
            </div>
          ))}
        </div>

        {/* Card */}
        <div className="rounded-xl bg-slate-800 p-8 shadow-xl">
          {/* Step 0: Welcome + module selection */}
          {step === 0 && (
            <div>
              <h2 className="mb-2 text-2xl font-bold text-white">Welcome to StackArr</h2>
              <p className="mb-6 text-slate-400">
                Your unified media management stack. Choose which modules to enable.
              </p>
              <div className="space-y-3">
                <label className="flex cursor-pointer items-center gap-4 rounded-lg border border-slate-600 p-4 transition-colors hover:border-blue-500">
                  <input
                    type="checkbox"
                    checked={enableTv}
                    onChange={(e) => setEnableTv(e.target.checked)}
                    className="h-5 w-5 rounded accent-blue-500"
                  />
                  <Tv size={24} className="text-blue-400" />
                  <div>
                    <div className="font-medium text-white">TV Series</div>
                    <div className="text-sm text-slate-400">Track and manage TV shows</div>
                  </div>
                </label>
                <label className="flex cursor-pointer items-center gap-4 rounded-lg border border-slate-600 p-4 transition-colors hover:border-blue-500">
                  <input
                    type="checkbox"
                    checked={enableMovies}
                    onChange={(e) => setEnableMovies(e.target.checked)}
                    className="h-5 w-5 rounded accent-blue-500"
                  />
                  <Film size={24} className="text-purple-400" />
                  <div>
                    <div className="font-medium text-white">Movies</div>
                    <div className="text-sm text-slate-400">Track and manage movies</div>
                  </div>
                </label>
              </div>
            </div>
          )}

          {/* Step 1: Download client type */}
          {step === 1 && (
            <div>
              <h2 className="mb-2 text-2xl font-bold text-white">Download Client</h2>
              <p className="mb-6 text-slate-400">
                How do you want to download media?
              </p>
              <div className="space-y-2">
                {(
                  [
                    { value: 'none', label: 'None', desc: 'Manual downloads only' },
                    { value: 'torrent', label: 'External Torrent', desc: 'Use a torrent client (qBittorrent, Transmission, etc.)' },
                    { value: 'usenet', label: 'External Usenet', desc: 'Use a usenet client (SABnzbd, NZBGet, etc.)' },
                    { value: 'both', label: 'Both', desc: 'Use both torrent and usenet clients' },
                  ] as const
                ).map((opt) => (
                  <label
                    key={opt.value}
                    className={`flex cursor-pointer items-center gap-3 rounded-lg border p-4 transition-colors ${
                      downloadClientType === opt.value
                        ? 'border-blue-500 bg-blue-500/10'
                        : 'border-slate-600 hover:border-slate-500'
                    }`}
                  >
                    <input
                      type="radio"
                      name="dlclient"
                      value={opt.value}
                      checked={downloadClientType === opt.value}
                      onChange={() => setDownloadClientType(opt.value)}
                      className="accent-blue-500"
                    />
                    <div>
                      <div className="font-medium text-white">{opt.label}</div>
                      <div className="text-sm text-slate-400">{opt.desc}</div>
                    </div>
                  </label>
                ))}
              </div>
            </div>
          )}

          {/* Step 2: Root folders */}
          {step === 2 && (
            <div>
              <h2 className="mb-2 text-2xl font-bold text-white">Root Folders</h2>
              <p className="mb-6 text-slate-400">
                Set the root directories for your media libraries.
              </p>
              <div className="space-y-4">
                {enableTv && (
                  <div>
                    <label className="mb-1.5 block text-sm font-medium text-slate-300">
                      TV Root Folder
                    </label>
                    <input
                      type="text"
                      value={tvRootFolder}
                      onChange={(e) => setTvRootFolder(e.target.value)}
                      className="w-full rounded-lg border border-slate-600 bg-slate-700 px-4 py-2.5 text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                      placeholder="/media/tv"
                    />
                  </div>
                )}
                {enableMovies && (
                  <div>
                    <label className="mb-1.5 block text-sm font-medium text-slate-300">
                      Movie Root Folder
                    </label>
                    <input
                      type="text"
                      value={movieRootFolder}
                      onChange={(e) => setMovieRootFolder(e.target.value)}
                      className="w-full rounded-lg border border-slate-600 bg-slate-700 px-4 py-2.5 text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                      placeholder="/media/movies"
                    />
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Step 3: Complete */}
          {step === 3 && (
            <div className="text-center">
              {setupMutation.isPending && (
                <div className="flex flex-col items-center gap-3">
                  <Loader2 size={48} className="animate-spin text-blue-500" />
                  <p className="text-slate-300">Setting up StackArr...</p>
                </div>
              )}
              {setupMutation.isError && (
                <div>
                  <div className="mb-4 text-red-400">
                    Setup failed: {setupMutation.error.message}
                  </div>
                  <button
                    onClick={handleFinish}
                    className="rounded-lg bg-blue-600 px-6 py-2.5 font-medium text-white hover:bg-blue-700 transition-colors"
                  >
                    Retry
                  </button>
                </div>
              )}
              {done && (
                <div>
                  <CheckCircle size={48} className="mx-auto mb-4 text-green-500" />
                  <h2 className="mb-2 text-2xl font-bold text-white">All Set!</h2>
                  <p className="mb-6 text-slate-400">
                    StackArr is ready to go. You can configure more in Settings.
                  </p>
                  <button
                    onClick={handleGoToApp}
                    className="rounded-lg bg-blue-600 px-6 py-2.5 font-medium text-white hover:bg-blue-700 transition-colors"
                  >
                    Get Started
                  </button>
                </div>
              )}
              {!setupMutation.isPending && !setupMutation.isError && !done && (
                <div>
                  <h2 className="mb-2 text-2xl font-bold text-white">Ready to Go</h2>
                  <p className="mb-6 text-slate-400">
                    Review your choices and finish setup.
                  </p>
                  <div className="mb-6 space-y-2 text-left text-sm">
                    <div className="flex justify-between rounded-lg bg-slate-700 px-4 py-2">
                      <span className="text-slate-400">Modules</span>
                      <span className="text-white">
                        {[enableTv && 'TV', enableMovies && 'Movies'].filter(Boolean).join(', ')}
                      </span>
                    </div>
                    <div className="flex justify-between rounded-lg bg-slate-700 px-4 py-2">
                      <span className="text-slate-400">Download Client</span>
                      <span className="text-white capitalize">{downloadClientType}</span>
                    </div>
                    {enableTv && (
                      <div className="flex justify-between rounded-lg bg-slate-700 px-4 py-2">
                        <span className="text-slate-400">TV Folder</span>
                        <span className="text-white font-mono text-xs">{tvRootFolder}</span>
                      </div>
                    )}
                    {enableMovies && (
                      <div className="flex justify-between rounded-lg bg-slate-700 px-4 py-2">
                        <span className="text-slate-400">Movie Folder</span>
                        <span className="text-white font-mono text-xs">{movieRootFolder}</span>
                      </div>
                    )}
                  </div>
                  <button
                    onClick={handleFinish}
                    className="rounded-lg bg-green-600 px-6 py-2.5 font-medium text-white hover:bg-green-700 transition-colors"
                  >
                    Finish Setup
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Navigation buttons */}
          {step < 3 && (
            <div className="mt-8 flex justify-between">
              <button
                onClick={() => setStep((s) => s - 1)}
                disabled={step === 0}
                className="flex items-center gap-1 rounded-lg px-4 py-2 text-sm font-medium text-slate-400 hover:text-white disabled:invisible transition-colors"
              >
                <ChevronLeft size={16} /> Back
              </button>
              <button
                onClick={() => setStep((s) => s + 1)}
                disabled={!canNext()}
                className="flex items-center gap-1 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
              >
                Next <ChevronRight size={16} />
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
