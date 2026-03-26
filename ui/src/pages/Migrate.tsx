import { useState, useRef } from 'react'
import { Database, Upload, Loader2, CheckCircle, XCircle, FileUp } from 'lucide-react'
import { useMigrate } from '../hooks/useApi'
import type { MigrationResult } from '../api/types'

export default function Migrate() {
  const mutation = useMigrate()
  const [sonarrFile, setSonarrFile] = useState<File | null>(null)
  const [radarrFile, setRadarrFile] = useState<File | null>(null)
  const [prowlarrFile, setProwlarrFile] = useState<File | null>(null)
  const formRef = useRef<HTMLFormElement>(null)

  const hasAnyFile = sonarrFile || radarrFile || prowlarrFile

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!hasAnyFile) return

    const formData = new FormData()
    if (sonarrFile) formData.append('sonarr', sonarrFile)
    if (radarrFile) formData.append('radarr', radarrFile)
    if (prowlarrFile) formData.append('prowlarr', prowlarrFile)

    mutation.mutate(formData)
  }

  const handleReset = () => {
    setSonarrFile(null)
    setRadarrFile(null)
    setProwlarrFile(null)
    mutation.reset()
    formRef.current?.reset()
  }

  return (
    <div>
      <h2 className="mb-6 text-2xl font-bold">Migration</h2>

      <div className="mx-auto max-w-2xl">
        <div className="rounded-xl bg-slate-800 p-8">
          <div className="mb-6 flex items-center gap-3">
            <Database size={24} className="text-blue-400" />
            <div>
              <h3 className="text-lg font-semibold">Import from Sonarr / Radarr / Prowlarr</h3>
              <p className="text-sm text-slate-400">
                Upload database files to migrate your existing library data into StackArr.
              </p>
            </div>
          </div>

          {/* Upload form */}
          {!mutation.isSuccess && (
            <form ref={formRef} onSubmit={handleSubmit}>
              <div className="space-y-4">
                <FileInput
                  label="sonarr.db"
                  description="Sonarr database file"
                  file={sonarrFile}
                  onFileChange={setSonarrFile}
                  accept=".db"
                />
                <FileInput
                  label="radarr.db"
                  description="Radarr database file"
                  file={radarrFile}
                  onFileChange={setRadarrFile}
                  accept=".db"
                />
                <FileInput
                  label="prowlarr.db"
                  description="Prowlarr database file (indexers)"
                  file={prowlarrFile}
                  onFileChange={setProwlarrFile}
                  accept=".db"
                />
              </div>

              {mutation.isError && (
                <div className="mt-4 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">
                  Migration failed: {mutation.error.message}
                </div>
              )}

              <div className="mt-6 flex gap-3">
                <button
                  type="submit"
                  disabled={!hasAnyFile || mutation.isPending}
                  className="flex items-center gap-2 rounded-lg bg-blue-600 px-6 py-2.5 font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
                >
                  {mutation.isPending ? (
                    <>
                      <Loader2 size={16} className="animate-spin" /> Migrating...
                    </>
                  ) : (
                    <>
                      <Upload size={16} /> Start Migration
                    </>
                  )}
                </button>
                {hasAnyFile && !mutation.isPending && (
                  <button
                    type="button"
                    onClick={handleReset}
                    className="rounded-lg bg-slate-700 px-4 py-2.5 font-medium text-slate-300 hover:bg-slate-600 transition-colors"
                  >
                    Clear
                  </button>
                )}
              </div>
            </form>
          )}

          {/* Results */}
          {mutation.isSuccess && mutation.data && (
            <MigrationReport result={mutation.data} onReset={handleReset} />
          )}
        </div>
      </div>
    </div>
  )
}

function FileInput({
  label,
  description,
  file,
  onFileChange,
  accept,
}: {
  label: string
  description: string
  file: File | null
  onFileChange: (f: File | null) => void
  accept: string
}) {
  const inputRef = useRef<HTMLInputElement>(null)

  return (
    <div
      onClick={() => inputRef.current?.click()}
      className="flex cursor-pointer items-center gap-4 rounded-lg border border-dashed border-slate-600 p-4 hover:border-blue-500 transition-colors"
    >
      <FileUp size={24} className={file ? 'text-blue-400' : 'text-slate-500'} />
      <div className="flex-1 min-w-0">
        <div className="font-medium text-white">{label}</div>
        <div className="text-xs text-slate-400">
          {file ? (
            <span className="text-blue-400">
              {file.name} ({(file.size / 1048576).toFixed(1)} MB)
            </span>
          ) : (
            description
          )}
        </div>
      </div>
      <input
        ref={inputRef}
        type="file"
        accept={accept}
        className="hidden"
        onChange={(e) => onFileChange(e.target.files?.[0] ?? null)}
      />
      {file && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            onFileChange(null)
            if (inputRef.current) inputRef.current.value = ''
          }}
          className="text-slate-400 hover:text-white"
        >
          <XCircle size={16} />
        </button>
      )}
    </div>
  )
}

function MigrationReport({ result, onReset }: { result: MigrationResult; onReset: () => void }) {
  return (
    <div>
      <div className="mb-4 flex items-center gap-2">
        {result.success ? (
          <>
            <CheckCircle size={24} className="text-green-500" />
            <span className="text-lg font-semibold text-green-400">Migration Complete</span>
          </>
        ) : (
          <>
            <XCircle size={24} className="text-red-500" />
            <span className="text-lg font-semibold text-red-400">Migration had errors</span>
          </>
        )}
      </div>

      <div className="mb-4 space-y-2">
        <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-3">
          <span className="text-slate-400">Series imported</span>
          <span className="font-medium text-white">{result.imported.series}</span>
        </div>
        <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-3">
          <span className="text-slate-400">Movies imported</span>
          <span className="font-medium text-white">{result.imported.movies}</span>
        </div>
        <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-3">
          <span className="text-slate-400">Indexers imported</span>
          <span className="font-medium text-white">{result.imported.indexers}</span>
        </div>
      </div>

      {result.errors.length > 0 && (
        <div className="mb-4">
          <h4 className="mb-2 text-sm font-medium text-red-400">Errors</h4>
          <div className="max-h-40 overflow-y-auto rounded-lg bg-slate-900 p-3">
            {result.errors.map((err, i) => (
              <div key={i} className="text-xs text-red-300">
                {err}
              </div>
            ))}
          </div>
        </div>
      )}

      <button
        onClick={onReset}
        className="rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
      >
        Start Over
      </button>
    </div>
  )
}
