import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import React, { useEffect } from "react";
import { formatDate } from "@/frontend/utils/format.utils";
import { DownloadableAppLogsModel } from "@/shared/model/downloadable-app-logs.model";
import { toast } from "sonner";
import { Actions } from "@/frontend/utils/nextjs-actions.utils";
import { exportLogsToFileForToday, getDownloadableLogs } from "./actions";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Download } from "lucide-react";
import FullLoadingSpinner from "@/components/ui/full-loading-spinnter";
import { Toast } from "@/frontend/utils/toast.utils";
import { DateUtils } from "@/shared/utils/date.utils";
import { useT } from "@/i18n";

export function LogsDownloadOverlay({
  children,
  appId,
  onClose
}: {
  children: React.ReactNode;
  appId: string;
  onClose?: () => void;
}) {

  const t = useT();
  const [logs, setLogs] = React.useState<DownloadableAppLogsModel[] | undefined>(undefined);
  const [isLoading, setIsLoading] = React.useState(false);

  const getLogsListAsync = async () => {
    setIsLoading(true);
    try {
      let logs: DownloadableAppLogsModel[] = await Actions.run(() => getDownloadableLogs(appId));
      const today = new Date();
      logs = logs.filter((log: DownloadableAppLogsModel) => !DateUtils.isSameDay(today, log.date));
      logs.unshift({
        appId: appId,
        date: new Date()
      });
      setLogs(logs);
    } catch (error) {
      toast.error(t('app.logs.downloadLoadFailed'));
    } finally {
      setIsLoading(false);
    }
  }

  const downloadLogFile = async (item: DownloadableAppLogsModel) => {
    try {
      setIsLoading(true);
      // check if item.date is today
      const today = new Date();
      if (DateUtils.isSameDay(today, item.date)) {
        const logsToOpen = await Toast.fromAction(() => exportLogsToFileForToday(appId));
        if (!logsToOpen.data) {
          throw new Error('No logs available for today');
        }
        item = logsToOpen.data;
      }
      window.open(`/api/logs-download?appId=${appId}&date=${item.date.toISOString()}`, '_blank');
    } finally {
      setIsLoading(false);
    }
  }

  useEffect(() => {
    getLogsListAsync();
  }, [appId]);

  return (
    <Dialog onOpenChange={(isO) => {
      if (!isO) {
        onClose?.();
      }
    }}>
      <DialogTrigger asChild>
        {children}
      </DialogTrigger>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>{t('app.logs.downloadTitle')}</DialogTitle>
          <DialogDescription>
            {t('app.logs.downloadDescription')}
          </DialogDescription>
        </DialogHeader>
        <ScrollArea className="max-h-[70vh]">
          {logs ? <Table>
            <TableCaption>{t('app.logs.count', { count: logs.length })}</TableCaption>
            <TableHeader>
              <TableRow>
                <TableHead>{t('common.date')}</TableHead>
                <TableHead></TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {logs.map((item, index) => (
                <TableRow key={index}>
                  <TableCell>{formatDate(item.date)}</TableCell>
                  <TableCell className="flex justify-end gap-2">
                    <Button variant="ghost" size="sm" onClick={() => downloadLogFile(item)} disabled={isLoading}>
                      <Download />
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table> : <FullLoadingSpinner />}
        </ScrollArea>
      </DialogContent>
    </Dialog>
  )
}
