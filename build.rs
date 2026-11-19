//! No Windows, embute o ícone e os metadados no executável (aparecem no Explorer e na barra de tarefas).

fn main() {
    println!("cargo:rerun-if-changed=assets/arbor.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/arbor.ico");
        res.set("ProductName", "arbor");
        res.set("FileDescription", "arbor — explorer de arquivos rápido");
        if let Err(e) = res.compile() {
            println!("cargo:warning=ícone do Windows não embutido: {e}");
        }
    }
}
