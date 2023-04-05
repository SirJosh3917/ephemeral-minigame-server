package com.sirn.server.commands;

import com.sirn.server.ServerPacketListener;
import com.sirn.transport.packets.Packet;
import com.sirn.transport.packets.RequestPacket;

import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;

import java.io.IOException;
import java.util.Arrays;
import java.util.UUID;

public class RequestCommand implements CommandExecutor {
    private final ServerPacketListener connection;

    public RequestCommand(ServerPacketListener connection) {
        this.connection = connection;
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        sender.sendMessage("request invoked with args " + Arrays.toString(args));

        String player = null;

        if (sender instanceof Player) {
            UUID uuid = ((Player) sender).getUniqueId();
            player = uuid.toString();
        }

        RequestPacket request = Packet.makeRequestMinigame(args[0], player);

		try {
			// TODO: this is wildly unsafe, as we could be in the middle of a period where
			// we loose connection to the controller, so the connection has already been
			// destroyed by the try-with-resources statement.
			//
			// Ignoring this problem for the time being :-)
	        this.connection.connection.write(request);
	        sender.sendMessage("you will be sent to a minigame server shortly (if you are not, try again)");
		} catch (IOException e) {
			sender.sendMessage("uh oh, big problem atm");
		}

        return true;
    }
}
