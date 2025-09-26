fn save_sysinfo(conn: &Connection, node_id: NodeID) {
    let mut sys = System::new_all();
    sys.refresh_all();

	let node = Node {
		id: node_id,
		name: System::host_name().unwrap_or_default(),
		kernel_version: System::kernel_version().unwrap_or_default(),
		total_memory: sys.total_memory(),
		you: true,
		system_name: System::name().unwrap_or_default(),
		os_version: System::os_version().unwrap_or_default(),
		created_at: Utc::now(),
		modified_at: Utc::now(),
		accessed_at: Utc::now(),
	};

	log::info!("saving node {:?}", node);

	save_node(conn, &node).unwrap();

    // Save current CPUs to database
    let mut current_cpu_names = Vec::new();
    for cpu in sys.cpus() {
        let name = cpu.name().to_string();
        current_cpu_names.push(name.clone());
        let cpu_entry = Cpu {
            node_id,
            name,
            usage: cpu.cpu_usage(),
            frequency: cpu.frequency() as u32,
            created_at: Utc::now(),
            modified_at: Utc::now(),
        };
        save_cpu(conn, &cpu_entry).unwrap();
    }
    // Remove any CPUs in DB that were not found in sysinfo
    remove_stale_cpus(conn, &node_id, &current_cpu_names).unwrap();

    // Save current disks to database
    let disks = Disks::new_with_refreshed_list();
    for disk in &disks {
        let name = disk.name().to_string_lossy().to_string();
        let total_space = disk.total_space();
        let available_space = disk.available_space();
        let usage = (total_space - available_space) as f32 / total_space as f32 * 100.0;
        let total_read_bytes = disk.usage().total_read_bytes;
        let total_written_bytes = disk.usage().total_written_bytes;
        let mount_path = disk.mount_point().to_string_lossy().to_string();
        let filesystem = disk.file_system().to_string_lossy().to_string();
        let readonly = disk.is_read_only();
        let removable = false;
        let kind = disk.kind().to_string();
        let disk_entry = Disk {
            node_id,
            name,
            usage,
            total_size: total_space,
            total_read_bytes,
            total_written_bytes,
            mount_path,
            filesystem,
            readonly,
            removable,
            kind,
            created_at: Utc::now(),
            modified_at: Utc::now(),
        };
        save_disk(conn, &disk_entry).unwrap();
    }
    // Save current network interfaces to database
    let networks = Networks::new_with_refreshed_list();
    for (name, data) in &networks {
		let ip = data.ip_networks().iter().next().map(|ip| ip.to_string()).unwrap_or_default();
        let interface_entry = Interface {
            node_id,
            name: name.clone(),
            ip,
            mac: data.mac_address().to_string(),
            loopback: false,
            linklocal: false,
            usage: data.total_transmitted() as f32,
            total_received: data.total_received() as u64,
            created_at: Utc::now(),
            modified_at: Utc::now(),
        };
        save_interface(conn, &interface_entry).unwrap();
    }

	let components = Components::new_with_refreshed_list();
	for component in &components {
		let label = component.label().to_string();
		let temperature = component.temperature();
		let max = component.max().unwrap_or(0.0);
		let critical = component.critical();

		let temp_entry = Temperature {
			node_id,
			label,
			temperature,
			max: Some(max),
			critical,
			created_at: Utc::now(),
			modified_at: Utc::now(),
		};
		save_temperature(conn, &temp_entry).unwrap();
	}
}