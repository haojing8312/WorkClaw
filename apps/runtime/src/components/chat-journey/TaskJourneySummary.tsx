import { DeliverySummaryCard } from "./DeliverySummaryCard";
import { TaskJourneyTimeline } from "./TaskJourneyTimeline";
import type { TaskJourneyViewModel } from "../chat-side-panel/view-model";

interface TaskJourneySummaryProps {
  model: TaskJourneyViewModel;
  workspace?: string;
  onViewFiles?: () => void;
  onOpenWorkspace?: () => void;
  onResumeFailedWork?: () => void;
}

export function TaskJourneySummary({
  model,
  workspace,
  onViewFiles,
  onOpenWorkspace,
  onResumeFailedWork,
}: TaskJourneySummaryProps) {
  return (
    <>
      <TaskJourneyTimeline model={model} />
      <DeliverySummaryCard
        model={model}
        workspace={workspace}
        onViewFiles={onViewFiles}
        onOpenWorkspace={onOpenWorkspace}
        onResumeFailedWork={onResumeFailedWork}
      />
    </>
  );
}
