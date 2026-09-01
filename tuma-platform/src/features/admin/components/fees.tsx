/**
 * Admin — Fees & Categories (coverage §7.8): settings-style page —
 * platform fee, category list with product counts, per-category
 * delivery-fee defaults, dirty-save bar, "Last edited by" line (P13).
 *
 * BACKEND GAP (G5): no platform-settings endpoints — GET/PUT
 * /v1/admin/platform-settings. The form is fully wired client-side;
 * Save is honest about the missing service instead of pretending.
 */
import { useMemo, useState } from 'react'
import { toast } from 'sonner'

import {
  Card,
  DirtySaveBar,
  Field,
  Input,
  PageHead,
} from '@/components/ds'
import { demoFeeSettings, type DemoFeeCategory } from '@/features/demo/seed'
import { dateTime, num } from '@/lib/format'

export function FeesPage() {
  const [platformFee, setPlatformFee] = useState(String(demoFeeSettings.platformFeePct))
  const [categories, setCategories] = useState<DemoFeeCategory[]>(
    demoFeeSettings.categories.map((c) => ({ ...c })),
  )
  const dirty = useMemo(
    () =>
      platformFee !== String(demoFeeSettings.platformFeePct) ||
      categories.some(
        (c, i) => c.deliveryFee !== demoFeeSettings.categories[i].deliveryFee,
      ),
    [platformFee, categories],
  )

  const setFee = (category: string, fee: string) => {
    setCategories((prev) =>
      prev.map((c) =>
        c.category === category
          ? { ...c, deliveryFee: Math.max(0, Number(fee.replace(/\D/g, '')) || 0) }
          : c,
      ),
    )
  }

  const save = () => {
    // BACKEND GAP (G5): PUT /v1/admin/platform-settings — until the
    // settings service exists, saving cannot pretend to persist.
    toast.error('Fee settings can’t be saved yet — this page is waiting on the platform settings service')
  }

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Fees & Categories"
        sub={
          <>
            Last edited by {demoFeeSettings.lastEditedBy},{' '}
            {dateTime(demoFeeSettings.lastEdited)}
          </>
        }
      />

      <Card className="max-w-200">
        <div className="text-[17px] font-bold">Platform fee</div>
        <div className="mt-4 max-w-60">
          <Field
            label="Platform fee (%)"
            help="Taken from each order's subtotal before the merchant's earnings"
          >
            <Input
              value={platformFee}
              inputMode="numeric"
              onChange={(e) =>
                setPlatformFee(e.target.value.replace(/[^\d]/g, '').slice(0, 2))
              }
            />
          </Field>
        </div>
      </Card>

      <Card className="max-w-200 p-0">
        <div className="flex items-center gap-3 px-6 pt-5 pb-3">
          <div className="text-[17px] font-bold">Delivery-fee defaults</div>
          <div className="ml-auto text-xs text-text3">
            What a new store of each category starts with
          </div>
        </div>
        {categories.map((c) => (
          <div
            key={c.category}
            className="flex flex-wrap items-center gap-3 border-t border-white/8 px-6 py-3"
          >
            <span className="min-w-40 flex-1 text-sm font-semibold">
              {c.category}
            </span>
            <span className="text-xs text-text3">
              {num(c.products)} products
            </span>
            <div className="w-40">
              <Field label="Default fee (RWF)">
                <Input
                  value={String(c.deliveryFee)}
                  inputMode="numeric"
                  onChange={(e) => setFee(c.category, e.target.value)}
                />
              </Field>
            </div>
          </div>
        ))}
        <div className="px-6 pb-5 pt-4 text-xs text-text3">
          Categories appear as merchants create stores — a category with
          stores can't be deleted, only renamed from the server.
        </div>
      </Card>

      <DirtySaveBar
        open={dirty}
        what="Fees & categories"
        onSave={save}
        onDiscard={() => {
          setPlatformFee(String(demoFeeSettings.platformFeePct))
          setCategories(demoFeeSettings.categories.map((c) => ({ ...c })))
        }}
      />
    </div>
  )
}
