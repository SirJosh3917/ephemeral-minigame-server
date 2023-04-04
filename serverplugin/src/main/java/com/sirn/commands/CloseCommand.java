package com.sirn.commands;

import com.sirn.controller_connection.HeadController;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;

public class CloseCommand implements CommandExecutor {
    private final HeadController connection;

    public CloseCommand(HeadController connection) {
        this.connection = connection;
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        sender.sendMessage("setting active state");

        if (args[0].equals("true")) {
            this.connection.setAcceptingPlayers(true);
        } else if (args[0].equals("false")){
            this.connection.setAcceptingPlayers(false);
        } else {
            sender.sendMessage("huh?");
        }

        return true;
    }
}
