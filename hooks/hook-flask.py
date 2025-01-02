from PyInstaller.utils.hooks import collect_data_files

# Include all Flask templates and static files
datas = collect_data_files('flask')
