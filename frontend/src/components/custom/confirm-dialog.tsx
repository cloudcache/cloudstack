'use client'

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import React from "react";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { useT } from "@/i18n";

export function ConfirmDialog() {
  const { isDialogOpen, data, closeDialog } = useConfirmDialog();
  const t = useT();
  if (!data) {
    return <></>;
  }

  return (
    <Dialog open={isDialogOpen} onOpenChange={closeDialog}>
      <DialogContent className="sm:max-w-[425px]">
        <DialogHeader>
          <DialogTitle>{data.title}</DialogTitle>
          <DialogDescription>
            {data.description}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          {data.okButton !== '' && <Button onClick={() => closeDialog(true)}>{data.okButton ?? t("common.ok")}</Button>}
          <Button variant="secondary" onClick={() => closeDialog(false)}>{data.cancelButton ?? t("common.cancel")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
