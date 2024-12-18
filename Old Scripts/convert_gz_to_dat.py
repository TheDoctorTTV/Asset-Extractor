import gzip
import os
import tkinter as tk
from tkinter import filedialog, messagebox

# Function to process .gz.asset files
def convert_files(input_folder, output_folder):
    if not os.path.exists(output_folder):
        os.makedirs(output_folder, exist_ok=True)

    for filename in os.listdir(input_folder):
        if filename.endswith(".gz.asset"):
            input_file = os.path.join(input_folder, filename)
            output_file = os.path.join(output_folder, filename.replace(".gz.asset", ".dat"))

            try:
                with gzip.open(input_file, "rb") as gz_file:
                    with open(output_file, "wb") as out_file:
                        out_file.write(gz_file.read())
                print(f"Converted: {filename} -> {os.path.basename(output_file)}")
            except Exception as e:
                print(f"Error processing {filename}: {str(e)}")

    messagebox.showinfo("Success", f"Conversion complete! Files saved in: {output_folder}")

# GUI Functions
def browse_input_folder():
    folder_selected = filedialog.askdirectory(title="Select Input Folder Containing .gz.asset Files")
    input_folder_var.set(folder_selected)

def browse_output_folder():
    folder_selected = filedialog.askdirectory(title="Select Output Folder for .dat Files")
    output_folder_var.set(folder_selected)

def start_conversion():
    input_folder = input_folder_var.get()
    output_folder = output_folder_var.get()

    if not input_folder or not output_folder:
        messagebox.showerror("Error", "Please select both input and output folders.")
        return

    convert_files(input_folder, output_folder)

# GUI Setup
root = tk.Tk()
root.title("GZ.Asset to .DAT Converter")

# Input Folder Selection
input_folder_var = tk.StringVar()
tk.Label(root, text="Input Folder:").pack(pady=5)
tk.Entry(root, textvariable=input_folder_var, width=50).pack(pady=5)
tk.Button(root, text="Browse", command=browse_input_folder).pack(pady=5)

# Output Folder Selection
output_folder_var = tk.StringVar()
tk.Label(root, text="Output Folder:").pack(pady=5)
tk.Entry(root, textvariable=output_folder_var, width=50).pack(pady=5)
tk.Button(root, text="Browse", command=browse_output_folder).pack(pady=5)

# Start Conversion Button
tk.Button(root, text="Start Conversion", command=start_conversion, font=("Arial", 12)).pack(pady=10)

# Run the GUI
root.mainloop()
