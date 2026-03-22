import { invoke } from "@tauri-apps/api/core";
import { useCallback, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { ModelConfig, SkillManifest } from "../types";

function getDefaultSkillId(skillList: SkillManifest[]): string | null {
  const builtin = skillList.find((item) => item.id === "builtin-general");
  if (builtin) {
    return builtin.id;
  }
  return skillList[0]?.id ?? null;
}

export function useCatalogDataCoordinator(options: {
  setSelectedSkillId: Dispatch<SetStateAction<string | null>>;
}) {
  const { setSelectedSkillId } = options;
  const [skills, setSkills] = useState<SkillManifest[]>([]);
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [searchConfigs, setSearchConfigs] = useState<ModelConfig[]>([]);
  const [hasHydratedModelConfigs, setHasHydratedModelConfigs] = useState(false);
  const [hasHydratedSearchConfigs, setHasHydratedSearchConfigs] =
    useState(false);

  const loadSkills = useCallback(async (): Promise<SkillManifest[]> => {
    const list = await invoke<SkillManifest[]>("list_skills");
    setSkills(list);
    setSelectedSkillId((prev) => {
      if (prev && list.some((item) => item.id === prev)) {
        return prev;
      }
      return getDefaultSkillId(list);
    });
    return list;
  }, [setSelectedSkillId]);

  const loadModels = useCallback(async () => {
    try {
      const list = await invoke<ModelConfig[]>("list_model_configs");
      setModels(list);
    } finally {
      setHasHydratedModelConfigs(true);
    }
  }, []);

  const loadSearchConfigs = useCallback(async () => {
    try {
      const list = await invoke<ModelConfig[]>("list_search_configs");
      setSearchConfigs(Array.isArray(list) ? list : []);
    } finally {
      setHasHydratedSearchConfigs(true);
    }
  }, []);

  return {
    hasHydratedModelConfigs,
    hasHydratedSearchConfigs,
    loadModels,
    loadSearchConfigs,
    loadSkills,
    models,
    searchConfigs,
    skills,
  };
}
