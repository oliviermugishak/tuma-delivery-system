import { PasswordCard } from './password-card'
import { ProfileCard } from './profile-card'

/**
 * Settings page for both wings: profile + password as stacked cards
 * (kanombe-sda house pattern). Shared because both rest on the same
 * endpoints (`PATCH /v1/me`, `POST /v1/auth/password`) and work identically
 * for admin and merchant accounts. Logout is not a setting — it pops out
 * of the user badge at the bottom of the sidebar.
 */
export function SettingsPanel() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="font-heading text-xl font-semibold">Settings</h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Your account on Tuma.
        </p>
      </div>
      <div className="max-w-3xl space-y-6">
        <ProfileCard />
        <PasswordCard />
      </div>
    </div>
  )
}
