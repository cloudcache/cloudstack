import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import React, { useEffect } from "react";
import { set } from "date-fns";
import { DeploymentInfoModel } from "@/shared/model/deployment-info.model";
import LogsStreamed from "@/components/custom/logs-streamed";
import { formatDateTime } from "@/frontend/utils/format.utils";
import BuildLogsStreamed from "@/components/custom/build-logs-streamed";
import { useT } from "@/i18n";

export function BuildLogsDialog({
  deploymentInfo,
  onClose
}: {
  deploymentInfo?: DeploymentInfoModel;
  onClose: () => void;
}) {
  const t = useT();

  if (!deploymentInfo) {
    return <></>;
  }

  return (
    <Dialog open={!!deploymentInfo} onOpenChange={(isO) => {
      onClose();
    }}>
      <DialogContent className="sm:max-w-[1300px]">
        <DialogHeader>
          <DialogTitle>{t('app.deployments.logsTitle')}</DialogTitle>
          <DialogDescription>
            {t('app.deployments.logsDescription', { time: formatDateTime(deploymentInfo.createdAt) })}
          </DialogDescription>
        </DialogHeader>
        <div >
          {!deploymentInfo.deploymentId && t('app.deployments.noBuildLogs')}
          {deploymentInfo.deploymentId && <BuildLogsStreamed deploymentId={deploymentInfo.deploymentId} />}
        </div>
      </DialogContent>
    </Dialog>
  )
}
