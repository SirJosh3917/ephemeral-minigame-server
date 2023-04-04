package com.sirn.minigames.debug;

import org.bukkit.plugin.java.JavaPlugin;

public class DebugMinigamePlugin extends JavaPlugin {
    @Override
    public void onEnable() {
        getLogger().info("The debug minigame has started!");

        getServer().getPluginCommand("endminigame").setExecutor(new EndMinigameCommand(getServer()));
        getServer().getPluginManager().registerEvents(new EventListener(), this);
    }
}
