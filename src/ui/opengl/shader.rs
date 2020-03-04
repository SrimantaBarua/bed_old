// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ffi::CStr;
use std::marker::PhantomData;
use std::ops::Drop;
use std::str;

use gl::types::{GLenum, GLint, GLuint};

use super::{ActiveGl, Mat4};

/// An active shader program
pub(in crate::ui) struct ActiveShaderProgram<'a, 'b>(GLuint, &'a PhantomData<&'b ()>);

impl<'a, 'b> ActiveShaderProgram<'a, 'b> {
    /// Set uniform matrix
    pub(in crate::ui) fn uniform_mat4f(&mut self, name: &CStr, mat: &Mat4) {
        unsafe {
            let loc = gl::GetUniformLocation(self.0, name.as_ptr());
            gl::UniformMatrix4fv(loc, 1, gl::FALSE, mat.as_ptr());
        }
    }

    pub(in crate::ui) fn uniform_1i(&mut self, name: &CStr, i: GLint) {
        unsafe {
            let loc = gl::GetUniformLocation(self.0, name.as_ptr());
            gl::Uniform1i(loc, i);
        }
    }
}

/// Handle to a shader program
pub(in crate::ui) struct ShaderProgram(GLuint);

impl ShaderProgram {
    /// Compile and link a shader from the given vertex and fragment shader source
    pub(in crate::ui) fn new(vsrc: &str, fsrc: &str) -> Result<ShaderProgram, String> {
        let mut success = 1;
        let mut len = 0;
        let mut info_log = [0; 512];
        let vshdr = Shader::new(vsrc, gl::VERTEX_SHADER, "vertex")?;
        let fshdr = Shader::new(fsrc, gl::FRAGMENT_SHADER, "fragment")?;
        unsafe {
            let id = gl::CreateProgram();
            gl::AttachShader(id, vshdr.0);
            gl::AttachShader(id, fshdr.0);
            gl::LinkProgram(id);
            gl::GetProgramiv(id, gl::LINK_STATUS, &mut success);
            if success == 0 {
                gl::GetProgramInfoLog(id, 512, &mut len, info_log.as_mut_ptr() as *mut i8);
                let info_str = str::from_utf8(&info_log[..(len as usize)]).unwrap();
                Err(format!("failed to link shader program: {}", info_str))
            } else {
                Ok(ShaderProgram(id))
            }
        }
    }

    /// Use shader program
    pub(in crate::ui) fn use_program<'a, 'b>(
        &'a mut self,
        _gl: &'b mut ActiveGl,
    ) -> ActiveShaderProgram<'a, 'b> {
        unsafe {
            gl::UseProgram(self.0);
        }
        ActiveShaderProgram(self.0, &PhantomData)
    }
}

impl Drop for ShaderProgram {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.0) }
    }
}

/// Handle to an individual shader compilation unit
struct Shader(GLuint);

impl Shader {
    /// Compile shader from source
    fn new(src: &str, typ: GLenum, name: &str) -> Result<Shader, String> {
        let mut success = 1;
        let mut len = 0;
        let mut info_log = [0; 512];
        unsafe {
            let id = gl::CreateShader(typ);
            gl::ShaderSource(id, 1, &(src.as_ptr() as *const i8), &(src.len() as i32));
            gl::CompileShader(id);
            gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
            if success == 0 {
                gl::GetShaderInfoLog(id, 512, &mut len, info_log.as_mut_ptr() as *mut i8);
                let info_str = str::from_utf8(&info_log[..(len as usize)]).unwrap();
                Err(format!("failed to compile {} shader: {}", name, info_str))
            } else {
                Ok(Shader(id))
            }
        }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe { gl::DeleteShader(self.0) }
    }
}
