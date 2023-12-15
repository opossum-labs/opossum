use std::{f64::{consts::PI, self}, time::Instant, collections::HashMap, env::current_dir, path::{Path, PathBuf}};

use approx::RelativeEq;
use itertools::{izip, iproduct, chain};
use nalgebra::{MatrixSliceXx1, MatrixXx1, MatrixXx2, MatrixXx3, Matrix1xX, DMatrix, MatrixSlice1xX, DVectorSlice, matrix, Matrix2, DVector, OMatrix, Matrix, vector};
use opossum::{error::{OpmResult, OpossumError}};
use plotters::{prelude::*, chart, backend::{RGBPixel, PixelFormat, self}};
use plotters::{backend::DrawingBackend, style::{RGBAColor, ShapeStyle, WHITE, TextStyle, IntoFont, RGBColor, HSLColor}, drawing::DrawingArea, 
coord::{Shift, cartesian::Cartesian2d, types::RangedCoordf64, ranged3d::Cartesian3d}, chart::{ChartContext, ChartBuilder}, series::LineSeries, element::Circle};
use colorous::Gradient;
use plotters_backend::{DrawingErrorKind, BackendCoord, BackendColor};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

fn linspace(start: f64, end: f64, num: usize) -> Matrix1xX<f64>{
    let mut linspace = Matrix1xX::<f64>::from_element(num, start);
    let bin_size = (end -start)/(num-1) as f64;
    for (i, val) in linspace.iter_mut().enumerate(){
        *val += bin_size * i as f64
    }
    linspace
}

fn linspace_u8(start: u8, step: u8, num: u8) -> Matrix1xX<u8>{
    let u8_iter = (start..(start+step*num)).step_by(step as usize).into_iter();
    Matrix1xX::<u8>::from_iterator(num as usize, u8_iter)
}

fn meshgrid_u8(x: &Matrix1xX<u8>, y: &Matrix1xX<u8>) -> (DMatrix<u8>,DMatrix<u8>){
    let x_len = x.len();
    let y_len = y.len();

    let mut x_mat = DMatrix::<u8>::zeros(y_len, x_len);
    let mut y_mat = DMatrix::<u8>::zeros(y_len, x_len);


    for x_id in 0..x_len{
        for y_id in 0..y_len{
            x_mat[(y_id, x_id)] = x[x_id];
            y_mat[(y_id, x_id)] = y[y_id];
        }
    };

    (x_mat, y_mat)

}
fn meshgrid(x: &Matrix1xX<f64>, y: &Matrix1xX<f64>) -> (DMatrix<f64>,DMatrix<f64>){
    let x_len = x.len();
    let y_len = y.len();

    let mut x_mat = DMatrix::<f64>::zeros(y_len, x_len);
    let mut y_mat = DMatrix::<f64>::zeros(y_len, x_len);


    for x_id in 0..x_len{
        for y_id in 0..y_len{
            x_mat[(y_id, x_id)] = x[x_id];
            y_mat[(y_id, x_id)] = y[y_id];
        }
    };

    (x_mat, y_mat)

}

#[derive( Debug, Clone, Copy)]
pub struct CGradient{
    gradient: Gradient,
}
impl CGradient{
    pub fn get_gradient(&self) -> Gradient{
        self.gradient
    }
}
impl Default for CGradient{
    fn default() -> Self{
        Self{ gradient: colorous::TURBO}
    }
}
#[derive( Debug, PartialEq, Clone, Copy)]
pub enum LabelPos{
    Top,
    Bottom,
    Left,
    Right
}

impl Default for LabelPos{
    fn default() -> Self {
        LabelPos::Left
    }
}

impl Into<LabelAreaPosition> for &LabelPos{
    fn into(self) -> LabelAreaPosition{
        match self{
            LabelPos::Top => LabelAreaPosition::Top,
            LabelPos::Bottom => LabelAreaPosition::Bottom,
            LabelPos::Left => LabelAreaPosition::Left,
            LabelPos::Right => LabelAreaPosition::Right,
        }
    }
    
}
#[derive(Clone)]
pub struct LabelDescription{
    label:      String,
    label_pos:  LabelPos,
}
impl LabelDescription{
    pub fn new(label: &str, label_pos: LabelPos) -> Self{
        Self { label: label.to_owned(), label_pos: label_pos}
    }

    pub fn y_default(&self) -> Self{
        LabelDescription::new("", LabelPos::Left)
    }

    pub fn set_label_pos(&mut self, pos: LabelPos) {
        self.label_pos = pos
    }

    pub fn set_label(&mut self, txt: &str) {
        self.label = txt.to_owned()
    }
}

#[derive(Clone)]
pub struct ColorBar{
    cmap:       Gradient,
    label:      LabelDescription,
}
impl ColorBar{
    pub fn new(cmap: Gradient, label: &str, label_pos: LabelPos) -> Self{
        Self { cmap: cmap, label: LabelDescription::new(label, label_pos)}
    }

    pub fn set_pos(&mut self, pos: LabelPos) {
        self.label.label_pos = pos
    }

    pub fn set_label(&mut self, txt: &str) {
        self.label.label = txt.to_owned()
    }
}
impl Default for ColorBar{
    fn default() -> Self {
        Self { cmap: colorous::INFERNO, label: LabelDescription::new("", LabelPos::Right)}
    }
}

#[derive(Clone)]
pub struct PlotBounds{
    x: Option<(f64,f64)>,
    y: Option<(f64,f64)>,
    z: Option<(f64,f64)>,
}

#[derive(Debug, Clone)]
pub enum PlotData{
    // ///Pairwise 2D data (e.g. x, y data) for scatter2D, Line2D. Data Structure as Matrix with N rows and two columns (x,y)
    // Dim2(MatrixXx2<f64>),
    // ///Triplet 3D data (e.g. x, y, z data) for scatter3D, Line3D. Data Structure as Matrix with N rows and three columns (x,y,z)
    // Dim3(MatrixXx3<f64>),
    // ///Vector of pairwise 2D data (e.g. x, y data) for MultiLine2D. Data Structure as Vector filled with Matrices with N rows and two columns (x,y)
    // MultiDim2(Vec<MatrixXx2<f64>>),
    // ///Vector of triplet 3D data (e.g. x, y, z data) for MultiLine3D. Data Structure as Vector filled with Matrices with N rows and three columns (x,y,z)
    // MultiDim3(Vec<MatrixXx3<f64>>),
    ColorMesh(DVector<f64>, DVector<f64>, DMatrix<f64>)
}

#[derive(Debug, Clone)]
pub enum PlotType {
    // ///Scatter plot in two dimensions for pairwise data
    // Scatter2D,
    // ///Scatter plot in three dimensions for 3D data
    // Scatter3D,
    // ///Line plot in two dimentions for pairwise data
    // Line2D,
    // ///Line plot in three dimensions for 3D data
    // Line3D,
    // ///Line plot for multiple lines, e.g. rays, in two dimentions with pairwise data
    // MultiLine2D,
    // ///Line plot for multiple lines, e.g. rays, in three dimentions with 3D data
    // MultiLine3D,
    // ///2D color plot of gridded data with color representing the amplitude over an x-y grid
    // #[default]
    ColorMesh(PlotParameters),
}

// impl Default for PlotType{
//     fn default(&self) -> Self{
//         PlotType::ColorMesh(PlotParameters::default())
//     }
// }

impl PlotType{
    fn get_plot_params(&self)-> &PlotParameters{
        match self{
            PlotType::ColorMesh(p) => p
        }
    }
    fn create_plot<B:DrawingBackend>(&self, backend: &DrawingArea<B, Shift>, plot: &mut Plot) -> OpmResult<()>{
        match self{
            PlotType::ColorMesh(_) => {
                _ = self.plot_color_mesh(plot, &backend);
                Ok(())
            },
            _ => Err(OpossumError::Other("Plottype not defined yet!".into()))
        }

    }

    pub fn plot(&self, plt_data: &PlotData) -> OpmResult<()>{
        let mut params: &PlotParameters = self.get_plot_params();
        params.check_validity()?;
        let path = params.get_fpath()?;
        let mut plot = Plot::new(plt_data, params);

        match params.get_backend()?{
            PltBackEnd::BMP =>{
                let backend = BitMapBackend::new(&path, plot.img_size).into_drawing_area();
                self.create_plot(&backend, &mut plot)
            }, 
            PltBackEnd::SVG =>{
                let path = plot.fpath.clone();
                let backend = SVGBackend::new(&path, plot.img_size).into_drawing_area();
                self.create_plot(&backend, &mut plot)
            },
        }
    }

    fn plot_color_mesh<B:DrawingBackend>(
        &self,
        plt:            &mut Plot, 
        root: &DrawingArea<B, Shift>
    ) -> OpmResult<()>{
    
        if let Some(PlotData::ColorMesh(x,y,dat)) = plt.get_data(){
    
            _ = root.fill(&WHITE);
            let (root_size_x, _) = root.dim_in_pixel();
            //split root for main plot and colorbar
            let (main_root, cbar_root) = root.split_horizontally(root_size_x as f64*0.85);
    
            
            let shape = dat.shape();
            let flattened_size = shape.0*shape.1;
            let dat_flat =     MatrixXx1::<f64>::from_iterator(flattened_size, dat.iter().map(|x| *x));
            
            let (x_min, x_max) = if plt.bounds.x.is_none(){
                plt.define_axis_bounds(&DVectorSlice::from(x), false, false)
            }
            else{
                plt.bounds.x.unwrap()
            };
            let (y_min, y_max) = if plt.bounds.y.is_none(){
                plt.define_axis_bounds(&DVectorSlice::from(y), false, false)
            }
            else{
                plt.bounds.y.unwrap()
            };
            let (z_min, z_max) = if plt.bounds.z.is_none(){
                plt.define_axis_bounds(&DVectorSlice::from(&dat_flat), false, false)
            }
            else{
                plt.bounds.z.unwrap()
            };
    
    
            //colorrbar. first because otherwise the xlabel of the main plot is cropped
            let mut chart = create_2d_plot_chart(
                &cbar_root,
                (0., 1.),
                (z_min, z_max),
                &"".into(),
                &plt.cbar.label.label,
                &plt.label[0].label_pos,
                &plt.cbar.label.label_pos,
                true, 
                false
                )?;
            
        
            let c_dat = linspace(z_min, z_max, 100).transpose();
            let d_mat = DMatrix::<f64>::from_columns(&[c_dat.clone(), c_dat]);
            let xxx = DVector::<f64>::from_vec(vec![0.,1.]);
            draw_2d_colormesh(
                &mut chart, 
                &xxx, 
                &linspace(z_min, z_max, 100).transpose(), 
                &d_mat,
                &plt.cbar.cmap,
                (z_min, z_max)
            );
    
            //main plot
            let mut chart = create_2d_plot_chart(
                &main_root,
                (x_min, x_max),
                (y_min, y_max),
                &plt.label[0].label,
                &plt.label[1].label,     
                &plt.label[0].label_pos,
                &plt.label[1].label_pos,       
                true, 
                true          
                )?;
    
            draw_2d_colormesh(
                &mut chart, 
                &x, 
                &y, 
                &dat,
                &plt.cbar.cmap,
                (z_min, z_max));
                            
        }
    
        
        root.present().unwrap();
        
        Ok(())
        }

        fn draw_2d_colormesh<'a, T: DrawingBackend>(
            &self,
            chart:          &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
            x:              &MatrixXx1<f64>,
            y:              &MatrixXx1<f64>,
            z:              &DMatrix<f64>,
            cmap:           &Gradient,
            cbounds:        (f64,f64)
        ){
            let mut x_dist = x[1] - x[0] ;
            if x_dist <= 2.*f64::EPSILON{
                x_dist = 1.
            }
            let y_dist = y[1] - y[0] ;
        
        
            // todo!("need to compare shapes!")
            //there will probably a more direct way to achieve the series without thisconversion to a vec<f64> when we can use nalgebra >=v0.32.
            //currently, clone is not implemented for matrix_iter in v0.30 which we use due to ncollide2d. Therefore, we go this way
            let (z_shape_rows, z_shape_cols) = z.shape();
        
            if z_shape_rows!= y.len() || z_shape_cols != x.len(){
                println!("Shapes of x,y and z do not match!");
                return;
            }
            let a:Vec<f64> = x.data.to_owned().into();
            let b:Vec<f64> = y.data.to_owned().into();
            let z_min = cbounds.0;
            
            let z_max: f64 = cbounds.1 - z_min;//z.max();
            let series = izip!(iproduct!(a,b),z).map(|((x,y),z)|{
                Rectangle::new(
                    [(x, y), (x + x_dist, y -y_dist)],
                    {
                        let cor = cmap.eval_continuous((z-z_min)/z_max);
                        let color = RGBAColor(
                            cor.r,
                            cor.g,
                            cor.b,
                            1.
                        );
                        Into::<ShapeStyle>::into(color).filled()
                    },
        
                )
            });
        
            chart.draw_series(series).unwrap();
        
        }
        
        fn create_2d_plot_chart<'a, T: DrawingBackend>(
            &self, 
            root: &'a DrawingArea<T, Shift>,
            x_bounds: (f64,f64),
            y_bounds: (f64,f64),
            xlabel: &String,
            ylabel: &String,
            xlabelpos: &LabelPos,
            ylabelpos: &LabelPos,
            y_ax: bool,
            x_ax: bool
            ) -> OpmResult<ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>> {
        
                root.fill(&WHITE).unwrap();
        
                let mut chart_builder = ChartBuilder::on(root);
                chart_builder.margin(15)
                .margin_top(40);
        
                let (x_px, y_px) = root.dim_in_pixel();     
        
                println!("{x_px}")               ;
                println!("{y_px}")               ;
        
                if y_ax{
                    chart_builder.set_label_area_size(ylabelpos.into(),90);
                }
                chart_builder.set_label_area_size(xlabelpos.into(), 65);
                
                let mut chart = chart_builder
                    .build_cartesian_2d(
                        x_bounds.0..x_bounds.1, 
                        y_bounds.0..y_bounds.1)
                    .unwrap();
        
                let mut mesh = chart
                    .configure_mesh();
        
                mesh.disable_mesh();
        
                if y_ax{
                    mesh.y_desc(ylabel);
                }
                else{
                    mesh.disable_y_axis();
                }
        
                if x_ax{
                    mesh.x_desc(xlabel);
                }
                else{
                    mesh.disable_x_axis();
                }
                
                mesh.label_style(TextStyle::from(("sans-serif", 30).into_font()))
                    .draw()
                    .unwrap();   
        
                Ok(chart)
            }
}

#[derive(EnumIter,  Debug, Clone)]
pub enum PlotArgs {
    XLabel(String),
    YLabel(String),

    XLabelPos(LabelPos),
    YLabelPos(LabelPos),

    Color(RGBAColor),
    CMap(CGradient),
    CBarLabel(String),
    CBarLabelPos(LabelPos),

    XLim(Option<(f64,f64)>),
    YLim(Option<(f64,f64)>),
    ZLim(Option<(f64,f64)>),

    FigSize((u32, u32)),
    FDir(String),
    FName(String),
    Backend(PltBackEnd),
}
#[derive(Default, Debug, Clone)]
pub struct PlotParameters{
    params: HashMap<String, PlotArgs>
}

impl PlotParameters{

    pub fn default() -> Self{
        let mut current_dir = current_dir().unwrap().to_str().unwrap().to_owned() + "\\";
        let mut i = 0;
        loop{
            let fpath = current_dir.clone() +format!("opossum_default_plot_{i}.png").as_str();
            let path = Path::new(&fpath);
            if !path.exists(){
                break
            }
            i += 1;
        }
        let mut plt_params = Self { params: HashMap::new() };
        plt_params
        .set(PlotArgs::Backend(PltBackEnd::BMP))
        .set(PlotArgs::XLabel("x".into()))
        .set(PlotArgs::XLabelPos(LabelPos::Bottom))
        .set(PlotArgs::YLabel("y".into()))
        .set(PlotArgs::YLabelPos(LabelPos::Left))
        .set(PlotArgs::CBarLabel("z value".into()))
        .set(PlotArgs::CBarLabelPos(LabelPos::Right))
        .set(PlotArgs::XLim(None))
        .set(PlotArgs::YLim(None))
        .set(PlotArgs::ZLim(None))
        .set(PlotArgs::CMap(CGradient::default()))
        .set(PlotArgs::Color(RGBAColor(255,0,0,1.)))
        .set(PlotArgs::FDir(current_dir))
        .set(PlotArgs::FName(format!("opossum_default_plot_{i}.png")))
        .set(PlotArgs::FigSize((1000,850)));

        plt_params
    }

    pub fn empty() -> Self{
        Self { params: HashMap::new() }
    }
    pub fn new(plt_args: Vec<PlotArgs>) -> Self{
        let mut p_i_params = Self { params: HashMap::new() };
        for plt_arg in plt_args{
            p_i_params.insert(&plt_arg);
        };

        for plt_arg in PlotArgs::iter(){
            if !p_i_params.check_if_set(&plt_arg){
                p_i_params.insert(&plt_arg);
            }
        };
        p_i_params
    }

    pub fn get_x_label(&self) -> OpmResult<String>{
        if let Some(PlotArgs::XLabel(x_label)) = self.params.get("xlabel"){
            Ok(x_label.clone())
        }
        else{
            Err(OpossumError::Other("xlabel argument not found!".into()))
        }
    }

    pub fn get_y_label(&self) -> OpmResult<String>{
        if let Some(PlotArgs::YLabel(y_label)) = self.params.get("ylabel"){
            Ok(y_label.clone())
        }
        else{
            Err(OpossumError::Other("ylabel argument not found!".into()))
        }
    }

    pub fn get_y_label_pos(&self) -> OpmResult<LabelPos>{
        if let Some(PlotArgs::YLabelPos(y_label_pos)) = self.params.get("ylabelpos"){
            Ok(y_label_pos.clone())
        }
        else{
            Err(OpossumError::Other("ylabelpos argument not found!".into()))
        }
    }

    pub fn get_x_label_pos(&self) -> OpmResult<LabelPos>{
        if let Some(PlotArgs::XLabelPos(x_label_pos)) = self.params.get("xlabelpos"){
            Ok(x_label_pos.clone())
        }
        else{
            Err(OpossumError::Other("xlabelpos argument not found!".into()))
        }
    }

    pub fn get_color(&self) -> OpmResult<RGBAColor>{
        if let Some(PlotArgs::Color(c)) = self.params.get("color"){
            Ok(c.clone())
        }
        else{
            Err(OpossumError::Other("color argument not found!".into()))
        }
    }
    pub fn get_backend(&self) -> OpmResult<PltBackEnd>{
        if let Some(PlotArgs::Backend(backend)) = self.params.get("backend"){
            Ok(backend.clone())
        }
        else{
            Err(OpossumError::Other("backend argument not found!".into()))
        }
    }

    pub fn get_cmap(&self) -> OpmResult<CGradient>{
        if let Some(PlotArgs::CMap(cmap)) = self.params.get("cmap"){
            Ok(cmap.clone())
        }
        else{
            Err(OpossumError::Other("cmap argument not found!".into()))
        }
    }
    pub fn get_cbar_label(&self) -> OpmResult<String>{
        if let Some(PlotArgs::CBarLabel(cbar_label)) = self.params.get("cbarlabel"){
            Ok(cbar_label.clone())
        }
        else{
            Err(OpossumError::Other("cbarlabel argument not found!".into()))
        }
    }
    pub fn get_cbar_label_pos(&self) -> OpmResult<LabelPos>{
        if let Some(PlotArgs::CBarLabelPos(cbar_label_pos)) = self.params.get("cbarlabelpos"){
            Ok(cbar_label_pos.clone())
        }
        else{
            Err(OpossumError::Other("cbarlabelpos argument not found!".into()))
        }
    }  

    pub fn get_fname(&self) -> OpmResult<String>{
        if let Some(PlotArgs::FName(fname)) = self.params.get("fname"){
            Ok(fname.clone())
        }
        else{
            Err(OpossumError::Other("fpath argument not found!".into()))
        }
    }   

    pub fn get_fpath(&self) -> OpmResult<String>{
        let fdir = self.get_fdir()?;
        let fname = self.get_fname()?;

        Ok(fdir + fname.as_str())
        
    }

    pub fn get_fdir(&self) -> OpmResult<String>{
        if let Some(PlotArgs::FDir(fdir)) = self.params.get("fdir"){
            Ok(fdir.clone())
        }
        else{
            Err(OpossumError::Other("fdir argument not found!".into()))
        }
    }   

    pub fn get_xlim(&self) -> OpmResult<Option<(f64,f64)>>{
        if let Some(PlotArgs::XLim(xlim)) = self.params.get("xlim"){
            Ok(xlim.clone())
        }
        else{
            Err(OpossumError::Other("xlim argument not found!".into()))
        }
    }

    pub fn get_ylim(&self) -> OpmResult<Option<(f64,f64)>>{
        if let Some(PlotArgs::YLim(ylim)) = self.params.get("ylim"){
            Ok(ylim.clone())
        }
        else{
            Err(OpossumError::Other("ylim argument not found!".into()))
        }
    }

    pub fn get_zlim(&self) -> OpmResult<Option<(f64,f64)>>{
        if let Some(PlotArgs::ZLim(zlim)) = self.params.get("zlim"){
            Ok(zlim.clone())
        }
        else{
            Err(OpossumError::Other("zlim argument not found!".into()))
        }
    }

    pub fn get_figsize(&self) -> OpmResult<(u32,u32)>{
        if let Some(PlotArgs::FigSize(figsize)) = self.params.get("figsize"){
            Ok(figsize.clone())
        }
        else{
            Err(OpossumError::Other("figsize argument not found!".into()))
        }
    }

    fn check_if_set(&self, plt_arg: &PlotArgs) -> bool{
        let mut found  = false;
        for param_val in self.params.values(){
            if std::mem::discriminant(param_val) == std::mem::discriminant(&plt_arg){
                found = true;
                break;
            }
        };
        found
    }

    fn get_plt_arg_key(&mut self, plt_arg: &PlotArgs) -> String{
        match  plt_arg{
            PlotArgs::XLabel(_) => "xlabel".to_owned(),
            PlotArgs::YLabel(_) => "ylabel".to_owned(),
            PlotArgs::XLabelPos(_) => "xlabelpos".to_owned(),
            PlotArgs::YLabelPos(_) => "ylabelpos".to_owned(),
            PlotArgs::Color(_) => "color".to_owned(),
            PlotArgs::CMap(_) => "cmap".to_owned(),
            PlotArgs::XLim(_) => "xlim".to_owned(),
            PlotArgs::YLim(_) => "ylim".to_owned(),
            PlotArgs::ZLim(_) => "zlim".to_owned(),
            PlotArgs::FigSize(_) => "figsize".to_owned(),
            PlotArgs::CBarLabelPos(_) => "cbarlabelpos".to_owned(),
            PlotArgs::CBarLabel(_) => "cbarlabel".to_owned(),
            PlotArgs::FDir(_) => "fdir".to_owned(),
            PlotArgs::FName(_) => "fname".to_owned(),
            PlotArgs::Backend(_) => "backend".to_owned(),
        }
    }

    pub fn set(&mut self, plt_arg: PlotArgs) -> &mut Self {
        let key = self.get_plt_arg_key(&plt_arg);
        if self.check_if_set(&plt_arg){
            self.params.remove_entry(&key);
        }
        self.insert(&plt_arg);
        self
    }

    pub fn check_validity(&self) -> OpmResult<()>{
        let backend = self.get_backend()?;
        let fname: String = self.get_fname()?;
        let fdir = self.get_fdir()? + "\\";
        let dir_path = Path::new(&fdir);

        let (valid_backend, err_msg) = self.check_backend_file_ext_compatibility(&fname, &backend);
        let mut err_path = "".to_owned();
        let mut valid_path = true;
        if !dir_path.exists(){
            err_path.push_str(format!("File-directory path \"{fdir}\\\" is not valid!\n\n").as_str());
            valid_path = false;
            println!("test");
        }



        if !(valid_path && valid_backend){
            err_path.push_str(format!("{err_msg}").as_str());
            Err(OpossumError::Other(err_path))
        }
        else{
            Ok(())
        }


    }

    fn check_backend_file_ext_compatibility(&self, path_fname: &String, backend: &PltBackEnd) -> (bool, &str){
        match backend{
            PltBackEnd::BMP =>{
                if path_fname.ends_with(".png") || path_fname.ends_with(".bmp") || path_fname.ends_with(".jpg"){
                    (true, "")
                }
                else{
                    (false, "Incompatible file extension for DrawingBackend: BitmapBackend! Choose \".jpg\", \".bmp\" or \".png\" for this type of backend!".into())
                }
            },
            PltBackEnd::SVG =>{
                if path_fname.ends_with(".svg"){
                    (true, "")
                }
                else{
                    (false, "Incompatible file extension for DrawingBackend: SVGBackend! Choose \".svg\"for this type of backend!".into())
                }
            },
        }
    }

    fn insert(&mut self, plt_arg: &PlotArgs){
        match  plt_arg{
            PlotArgs::XLabel(_) =>      self.params.insert("xlabel".to_owned(), plt_arg.clone()),
            PlotArgs::YLabel(_) =>      self.params.insert("ylabel".to_owned(), plt_arg.clone()),
            PlotArgs::XLabelPos(_) =>   self.params.insert("xlabelpos".to_owned(), plt_arg.clone()),
            PlotArgs::YLabelPos(_) =>   self.params.insert("ylabelpos".to_owned(), plt_arg.clone()),
            PlotArgs::Color(_) =>       self.params.insert("color".to_owned(), plt_arg.clone()),
            PlotArgs::CMap(_) =>        self.params.insert("cmap".to_owned(), plt_arg.clone()),
            PlotArgs::XLim(_) =>        self.params.insert("xlim".to_owned(), plt_arg.clone()),
            PlotArgs::YLim(_) =>        self.params.insert("ylim".to_owned(), plt_arg.clone()),
            PlotArgs::ZLim(_) =>        self.params.insert("zlim".to_owned(), plt_arg.clone()),
            PlotArgs::FigSize(_) =>     self.params.insert("figsize".to_owned(), plt_arg.clone()),
            PlotArgs::CBarLabelPos(_) => self.params.insert("cbarlabelpos".to_owned(), plt_arg.clone()),
            PlotArgs::CBarLabel(_) =>   self.params.insert("cbarlabel".to_owned(), plt_arg.clone()),
            PlotArgs::FDir(dir) => self.params.insert("fdir".to_owned(), plt_arg.clone()),
            PlotArgs::FName(n) =>self.params.insert("fname".to_owned(), plt_arg.clone()),
            PlotArgs::Backend(b) => self.params.insert("backend".to_owned(), plt_arg.clone()),
        };
    }

}

#[derive(Clone)]
pub struct Plot{
    // plt_type:   PlotType,
    label:      [LabelDescription;2],
    cbar:       ColorBar,
    data:       Option<PlotData>,
    bounds:     PlotBounds,
    fpath:      String,
    img_size:   (u32, u32),
    // root:       Option<DrawingArea<BitMapBackend<'a>, Shift>>

}

// pub struct Plot<'a>{
//     plt_info:   PlotInfo, 
//     root:       Option<PltBackEndRoot<'a>>,//DrawingArea<BitMapBackend<'a>, Shift>,
// }

// impl <'a> Plot<'a>{    
//     fn create_plot_root(&self, fpath: &'a str, img_size: (u32, u32)) -> Option<PltBackEndRoot>{
//         Some(PltBackEndRoot::BMP(BitMapBackend::new(fpath, img_size).into_drawing_area()))
//     }

//     fn new(plt_info: &'a PlotInfo) -> Self{
//         Self{
//             plt_info: plt_info.to_owned(),
//             root: None,
//         }
//     }
// }

impl TryFrom<&PlotParameters> for Plot{
    type Error = OpossumError;

    fn try_from(p_i_params: &PlotParameters) -> OpmResult<Self> {

        // let plt_type = p_i_params.get_plt_type()?;
        let cmap_gradient = p_i_params.get_cmap()?;
        let cbar_label_str = p_i_params.get_cbar_label()?;
        let cbar_label_pos = p_i_params.get_cbar_label_pos()?;
        let color = p_i_params.get_color()?;
        let fig_size = p_i_params.get_figsize()?;
        let xlim = p_i_params.get_xlim()?;
        let ylim = p_i_params.get_ylim()?;
        let zlim = p_i_params.get_zlim()?;
        let xlabel_str = p_i_params.get_x_label()?;
        let ylabel_str = p_i_params.get_y_label()?;
        let xlabel_pos = p_i_params.get_x_label_pos()?;
        let ylabel_pos = p_i_params.get_y_label_pos()?;
        let fdir = p_i_params.get_fdir()?;
        let fname = p_i_params.get_fname()?;
        
        let xlabel = LabelDescription::new(&xlabel_str, xlabel_pos);
        let ylabel = LabelDescription::new(&ylabel_str, ylabel_pos);
        let cbarlabel = LabelDescription::new(&cbar_label_str, cbar_label_pos);

        let cbar = ColorBar{
                cmap: cmap_gradient.get_gradient(),
                label: cbarlabel,
        };
            

        let mut plt_info: Plot = Self {
            // plt_type: *plt_type,
            label: [xlabel, ylabel],
            cbar: cbar,
            data: None,
            bounds: PlotBounds{x: xlim, y: ylim, z: zlim},
            fpath: fdir + fname.as_str(),
            img_size: fig_size,
        };


        Ok(plt_info)
    }
}


// impl Plotting for PlotType{
//     fn plot(
//         &self, 
//         plt_data: &PlotData, 
//         plot_params: PlotParameters,
//     ) -> OpmResult<()> {

//         let mut plot = Plot::new(plt_data, plot_params);

//         match self{
//             PlotType::ColorMesh => {
//                 let path = plot.fpath.clone();
//                 let backend = BitMapBackend::new(&path, plot.img_size).into_drawing_area();
//                 _ = self.plot_color_mesh(&mut plot, &backend);
//                 Ok(())
//             },
//             _ => Err(OpossumError::Other("Plottype notdefined yet!".into()))
//         }

//         // Ok(())

//     }
// }



impl Plot{
    pub fn new(plt_data: &PlotData, plt_params: &PlotParameters) -> Self{
        let mut plot = Plot::try_from(plt_params).unwrap();    
        plot.set_data(plt_data.clone());

        plot
    }

    pub fn set_data(&mut self, data: PlotData){
        self.data = Some(data)
    }

    pub fn get_data(&self) -> Option<&PlotData>{
        self.data.as_ref()
    }

    pub fn define_axes_bounds(
        &mut self, 
    )-> OpmResult<()>{
        if let Some(dat) = &self.data{
            match dat{
                PlotData::ColorMesh(x, y, dat) => {
                    if self.bounds.x.is_none(){
                        self.bounds.x = Some(self.define_axis_bounds(&DVectorSlice::from(&x.transpose()), false, false));
                    }
                    if self.bounds.y.is_none(){
                        self.bounds.y = Some(self.define_axis_bounds(&DVectorSlice::from(&y.transpose()), false, false));
                    }
                    self.bounds.z = None;
                    Ok(())
                }
                _=> Err(OpossumError::Other("Not defined yet!".into()))
            }
        }
        else{
            Err(OpossumError::Other("No plot data defined!".into()))
        }

    }

    fn get_min_max_range(&self, ax_vals: &DVectorSlice<f64>) -> (f64,f64,f64){
        let max_val = ax_vals.max();
        let min_val = ax_vals.min();
        let mut ax_range = max_val - min_val;

        //check if mininum and maximum value are approximately equal. if so, take max value as range
        if max_val.relative_eq(&min_val, f64::EPSILON, f64::EPSILON)
        {
            ax_range = max_val;
        };

        //check if for some reason maximum is 0, then set to 1, so that the axis spans at least some distance
        if ax_range < f64::EPSILON
        {
            ax_range = 1.;
        };

        (ax_range, min_val, max_val)
    }

    fn define_axis_bounds(    
        &self, 
        x: &DVectorSlice<f64>, 
        expand_min: bool, 
        expand_max: bool
    ) -> (f64, f64){
    
        //filter out every infinite value and every NaN
        let x_filtered = MatrixXx1::from(x.iter().cloned().filter(|x| !x.is_nan() & x.is_finite()).collect::<Vec<f64>>());
    
        //this only happens if all entries in this matrix are either infinite or NAN
        let (x_range, x_min, x_max) =  if x_filtered.len() == 0{
            (1., 0., 1.)
        }
        else{
            //get maximum and minimum of the axis
            self.get_min_max_range(&DVectorSlice::from(&x_filtered))    
        };
    
        //add spacing to the edges if defined
        let add_range_fac = 0.1 ;
        let expand_min_fac = expand_min as i32 as f64;
        let expand_max_fac = expand_max as i32 as f64;
        
        let range_start = x_min-x_range*add_range_fac*expand_min_fac;
        let range_end = x_max+x_range*add_range_fac*expand_max_fac;

        (range_start, range_end) 
    
    }

    // fn create_plot_root(
    //     &'a mut self
    // ){
    //         self.root = Some(BitMapBackend::new(self.fpath.as_str(), self.img_size).into_drawing_area().clone());
            
    //     }

}


#[derive(Clone, Debug, Default)]
pub enum PltBackEnd
    {    
    #[default]
    BMP,
    SVG
}

// impl <'a>PltBackEnd<'a>{
//     pub fn get_bmp(&'a self) -> OpmResult<&'a DrawingArea<BitMapBackend<'_>, Shift>>{
//         if let PltBackEnd::BMP(backend)= self{
//             Ok(backend)
//         }
//         else{
//             Err(OpossumError::Other("Not a BitmapBackend!".into()))
//         }
//     }
//     pub fn get_svg(&'a self) -> OpmResult<&'a DrawingArea<SVGBackend<'_>, Shift>>{
//         if let PltBackEnd::SVG(backend)= self{
//             Ok(backend)
//         }
//         else{
//             Err(OpossumError::Other("Not an SVGBackend!".into()))
//         }
//     }
// }
// #[derive(Clone)]
// enum PltBackEndRoot<'a, P = RGBPixel>
//     where
//     P:PixelFormat
//     {    
//     BMP(DrawingArea<BitMapBackend<'a, P>, Shift>),
//     SVG(DrawingArea<SVGBackend<'a>, Shift>)
// }


// impl <'a> PltBackEndRoot<'a>{
//     pub fn fill(&'a self, color: &RGBColor){
//         match self{
//             PltBackEndRoot::BMP(root) => {root.fill(color);},
//             PltBackEndRoot::SVG(root) => {root.fill(color);},
//         };
//     }

//     pub fn present(&'a self){
//         match self{
//             PltBackEndRoot::BMP(root) => root.present().unwrap(),
//             PltBackEndRoot::SVG(root) => root.present().unwrap(),
//         };
//     }

//     pub fn split_horizontally(&'a self, ratio: f64) -> (PltBackEndRoot<'a>, PltBackEndRoot<'a>){
//         match self{
//             PltBackEndRoot::BMP(root) => {
//                 let (l,r) = root.split_horizontally(ratio);
//                 (PltBackEndRoot::BMP(l), PltBackEndRoot::BMP(r))
//             },
//             PltBackEndRoot::SVG(root) => {
//                 let (l,r) = root.split_horizontally(ratio);
//                 (PltBackEndRoot::SVG(l), PltBackEndRoot::SVG(r))
//             },
//         }
//     }

//     pub fn split_vertically(&'a self, ratio: f64) -> (PltBackEndRoot<'a>, PltBackEndRoot<'a>){
//         match self{
//             PltBackEndRoot::BMP(root) => {
//                 let (t,b) = root.split_vertically(ratio);
//                 (PltBackEndRoot::BMP(t), PltBackEndRoot::BMP(b))
//             },
//             PltBackEndRoot::SVG(root) => {
//                 let (t,b) = root.split_vertically(ratio);
//                 (PltBackEndRoot::SVG(t), PltBackEndRoot::SVG(b))
//             },
//         }
//     }

//     pub fn dim_in_pixel(&'a self) -> (u32,u32){
//         match self{
//             PltBackEndRoot::BMP(root) => root.dim_in_pixel(),
//             PltBackEndRoot::SVG(root) => root.dim_in_pixel(),
//         }
//     }
// }


pub trait Plotting {
    fn plot(
        &self, 
        plt_data: &PlotData, 
        plot_info_params: PlotParameters,
    ) -> OpmResult<()>;
}

// fn plot_2d_line<B: DrawingBackend>(
//     &self,
//     plt_data:       &PlotData, 
//     marker_color:   RGBAColor, 
//     expand_bounds:  Vec<[bool;2]>,
//     xlabel: &str, 
//     ylabel: &str,
//     root: &DrawingArea<B, Shift>
// ) -> OpmResult<()>{

//     if let PlotData::Dim2(dat) = plt_data{
//         let (x_min, x_max) = define_axis_bounds(&dat.column(0), true, true);
//         let (y_min, y_max) = define_axis_bounds(&dat.column(1), true, true);

//         let mut chart = self.create_2d_plot_chart(
//             &root,
//             [x_min, x_max, y_min, y_max],
//             xlabel,
//             ylabel
//             )?;
//             self.draw_line(&mut chart, &dat.column(0), &dat.column(1), &marker_color);
//         }

//         root.present().unwrap();

//         Ok(())
//     }

//     fn plot_2d_scatter<B: DrawingBackend>(
//         &self,
//         plt_data:       &PlotData, 
//         marker_color:   RGBAColor, 
//         expand_bounds:  Vec<[bool;2]>,
//         xlabel: &str, 
//         ylabel: &str,
//         root: &DrawingArea<B, Shift>
//     ) -> OpmResult<()>{

//         if let PlotData::Dim2(dat) = plt_data{
//             let (x_min, x_max) = define_axis_bounds(&dat.column(0), true, true);
//             let (y_min, y_max) = define_axis_bounds(&dat.column(1), true, true);

//             let mut chart = self.create_2d_plot_chart(
//                 &root,
//                 [x_min, x_max, y_min, y_max],
//                 xlabel,
//                 ylabel
//                 )?;

//                 self.draw_points(&mut chart, &dat.column(0), &dat.column(1), &marker_color);
//         }

//         root.present().unwrap();
//         Ok(())
//     }

//     fn draw_line<'a, T: DrawingBackend>(
//         &self,
//         chart:      &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
//         x:          &MatrixSliceXx1<f64>,
//         y:          &MatrixSliceXx1<f64>,
//         line_color: &RGBAColor
//     ){
//         chart
//             .draw_series(LineSeries::new(
//                 izip!(x, y)
//                      .map(|xy| (*xy.0, *xy.1)), 
//                line_color)
//             ).unwrap();
//     }

//     fn draw_points<'a, T: DrawingBackend>(
//         &self,
//         chart:          &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
//         x:              &MatrixSliceXx1<f64>,
//         y:              &MatrixSliceXx1<f64>,
//         marker_color:   &RGBAColor
//     ){
//         chart.draw_series(
//             izip!(x, y).map(|x| Circle::new((*x.0, *x.1), 5, Into::<ShapeStyle>::into(marker_color).filled())),
//         ).unwrap();
//     }

//     fn draw_2d_colormesh<'a, T: DrawingBackend>(
//         &self,
//         chart:          &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
//         x:              &MatrixXx1<f64>,
//         y:              &MatrixXx1<f64>,
//         z:              &DMatrix<f64>,
//         cmap:           &Gradient
//     ){
//         let mut x_dist = x[1] - x[0] ;
//         if x_dist <= 2.*f64::EPSILON{
//             x_dist = 1.
//         }
//         let y_dist = y[1] - y[0] ;


//         // todo!("need to compare shapes!")
//         //there will probably a more direct way to achieve the series without thisconversion to a vec<f64> when we can use nalgebra >=v0.32.
//         //currently, clone is not implemented for matrix_iter in v0.30 which we use due to ncollide2d. Therefore, we go this way
//         let a:Vec<f64> = x.data.to_owned().into();
//         let b:Vec<f64> = y.data.to_owned().into();
//         let z_max: f64 = z.max();

//         let series = izip!(iproduct!(a,b),z).map(|((x,y),z)|{
//             Rectangle::new(
//                 [(x, y), (x + x_dist, y -y_dist)],
//                 {
//                     let cor = cmap.eval_continuous(z/z_max);
//                     let color = RGBAColor(
//                         cor.r,
//                         cor.g,
//                         cor.b,
//                         1.
//                     );
//                     Into::<ShapeStyle>::into(color).filled()
//                 },

//             )
//         });

//         chart.draw_series(series).unwrap();

//     }

//     fn plot_color_mesh<B: DrawingBackend>(
//         &self,
//         plt_data:       &PlotData, 
//         cmap:           &Gradient, 
//         xlabel:         &str, 
//         ylabel:         &str,
//         zlabel:         &str,
//         root:           &DrawingArea<B, Shift>,
//     ) -> OpmResult<()>{

//         if let PlotData::ColorMesh(x,y,dat) = plt_data{

//             let (root_size_x, _) = root.dim_in_pixel();
//             //split root for main plot and colorbar
//             let (left, right) = root.split_horizontally(root_size_x*7/10);

//             let shape = dat.shape();
//             let flattened_size = shape.0*shape.1;
//             let dat_flat =     MatrixXx1::<f64>::from_iterator(flattened_size, dat.iter().map(|x| *x));
//             let (x_min, x_max) = define_axis_bounds(&DVectorSlice::from(x), false, false);
//             let (y_min, y_max) = define_axis_bounds(&DVectorSlice::from(y), false, false);
//             let (z_min, z_max) = define_axis_bounds(&DVectorSlice::from(&dat_flat), false, false);

//             //main plot
//             let mut chart = self.create_2d_plot_chart(
//                 &left,
//                 [x_min, x_max, y_min, y_max],
//                 xlabel,
//                 ylabel,
//                 )?;

//                 self.draw_2d_colormesh(
//                     &mut chart, 
//                     &x, 
//                     &y, 
//                     &dat,
//                     cmap);

//             //colorbar
//             let mut chart: ChartContext<'_, B, Cartesian2d<RangedCoordf64, RangedCoordf64>> = self.create_2d_plot_chart(
//                 &right,
//                 [0., 1., z_min, z_max],
//                 "",
//                 "colorbar",
//                 )?;

            
            

//             let c_dat = linspace(z_min, z_max, 100).transpose();
//             let d_mat = DMatrix::<f64>::from_columns(&[c_dat.clone(), c_dat]);
//             let xxx = DVector::<f64>::from_vec(vec![0.,1.]);
//             self.draw_2d_colormesh(
//                 &mut chart, 
//                 &xxx, 
//                 &linspace(z_min, z_max, 100).transpose(), 
//                 &d_mat,
//                 cmap);
//         }

//         root.present().unwrap();
//         Ok(())
//     }

//     fn create_3d_plot_chart<'a, T: DrawingBackend>(
//         &self,
//         root: &'a DrawingArea<T, plotters::coord::Shift>,
//         bounds: [f64; 6], 
//         pitch: f64,
//         yaw: f64,
//         xlabel: &str,
//         ylabel: &str,
//         zlabel: &str
//     ) -> OpmResult<ChartContext<'a, T, Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>>> {

//         root.fill(&WHITE).unwrap();

//         let mut chart = ChartBuilder::on(root)
//             .margin(20)
//             .set_all_label_area_size(100)
//             .build_cartesian_3d(
//                 bounds[0]..bounds[1], 
//                 bounds[2]..bounds[3], 
//                 bounds[4]..bounds[5])
//             .unwrap();

//         chart.with_projection(|mut pb: plotters::coord::ranged3d::ProjectionMatrixBuilder| {
//                 pb.pitch = pitch*PI;
//                 pb.yaw = yaw*PI;
//                 pb.scale = 0.7;
//                 pb.into_matrix()
//             });

//         chart
//             .configure_axes()
//             .draw()
//             .unwrap();        

//         Ok(chart)
//     }

//     fn create_2d_plot_chart<'a, T: DrawingBackend>(
//         &self, 
//         root: &'a DrawingArea<T, Shift>,
//         bounds: [f64; 4], 
//         xlabel: &str, 
//         ylabel: &str
//         ) -> OpmResult<ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>> {
    
//             root.fill(&WHITE).unwrap();
    
//             let mut chart = ChartBuilder::on(root)
//                 .margin(5)
//                 .x_label_area_size(100)
//                 .y_label_area_size(100)
//                 .build_cartesian_2d(
//                     bounds[0]..bounds[1], 
//                     bounds[2]..bounds[3])
//                 .unwrap();
    
//             chart
//                 .configure_mesh()
//                 .disable_mesh()
//                 .x_desc(xlabel)
//                 .y_desc(ylabel)
//                 .label_style(TextStyle::from(("sans-serif", 30).into_font()))
//                 .draw()
//                 .unwrap();   
    
//             Ok(chart)
//         }

// }

    fn plot_color_mesh<B: DrawingBackend>(
        plt_data:       &PlotData, 
        cmap:           &Gradient, 
        xlabel:         &String, 
        ylabel:         &String,
        xlabelpos:      &LabelPos,
        ylabelpos:      &LabelPos,
        cbarlabelpos:   &LabelPos,
        cbarlabel:      &String,
        root:           &DrawingArea<B, Shift>,
    ) -> OpmResult<()>{

        if let PlotData::ColorMesh(x,y,dat) = plt_data{

            let (root_size_x, _) = root.dim_in_pixel();
            //split root for main plot and colorbar
            let (left, right) = root.split_horizontally(root_size_x*7/10);

            let shape = dat.shape();
            let flattened_size = shape.0*shape.1;
            let dat_flat =     MatrixXx1::<f64>::from_iterator(flattened_size, dat.iter().map(|x| *x));
            //main plot

            let mut chart = create_2d_plot_chart(
                &left,
                (-50., 50.),
                (-50., 50.),
                xlabel,
                ylabel,
                xlabelpos,
                ylabelpos,
                true,
                true
                )?;

                draw_2d_colormesh(
                    &mut chart, 
                    &x, 
                    &y, 
                    &dat,
                    cmap,
                    (0., 1.));

            //colorbar
            let labx = "".to_owned();
            let mut chart: ChartContext<'_, B, Cartesian2d<RangedCoordf64, RangedCoordf64>> = create_2d_plot_chart(
                &right,
                (0., 1.), 
                (0., 1.),
                &labx,
                cbarlabel,
                &LabelPos::Bottom,
                cbarlabelpos,
                true,
                false
                )?;

        
            let c_dat = linspace(0., 1., 100).transpose();
            let d_mat = DMatrix::<f64>::from_columns(&[c_dat.clone(), c_dat]);
            let xxx = DVector::<f64>::from_vec(vec![0.,1.]);
            draw_2d_colormesh(
                &mut chart, 
                &xxx, 
                &linspace(0., 1., 100).transpose(), 
                &d_mat,
                cmap,
                (0., 1.));
        }

        root.present().unwrap();
        Ok(())
    }

fn main() -> OpmResult<()>{

    let x = linspace(-50., 50., 101);
    let y = linspace(-50., 50., 101);
    let sigma = 10.;

    let (xx,yy) = meshgrid(&x, &y);
    let gaussian = (-0.5*(xx.map(|x| x.powi(2)) + yy.map(|y| y.powi(2)))/f64::powi(sigma,2)).map(|x| x.exp())*2.;

    let f_path = "./opossum/playground/plot_test.png";

    // let root: DrawingArea<BitMapBackend<'_>, Shift> = BitMapBackend::new(f_path, (800,800)).into_drawing_area();
    //     root.fill(&WHITE)
            // .map_err(|e| format!("filling plot background failed: {e}")).unwrap();

    let plt_data = PlotData::ColorMesh(x.transpose(),y.transpose(),gaussian);
    let gradient:Gradient = colorous::TURBO;

    let mut p_info_params = PlotParameters::default();

    // p_info_params.set(PlotArgs::CMap(CGradient{gradient:colorous::VIRIDIS}));
    //     .set(PlotArgs::CBarLabel("cbar label".into()))
    //     .set(PlotArgs::CBarLabelPos(LabelPos::Right))

    p_info_params.set(PlotArgs::Backend(PltBackEnd::BMP))
    .set(PlotArgs::FName("plot_test.png".into()))
    .set(PlotArgs::FDir("./opossum/playground/".into()));


    //

    //     .set(PlotArgs::FigSize((1000,850)))
    //     .set(PlotArgs::XLabel("x label".into()))
    //     .set(PlotArgs::YLabel("y label".into()))
    //     .set(PlotArgs::XLabelPos(LabelPos::Bottom))
    //     .set(PlotArgs::YLabelPos(LabelPos::Left))
    //     .set(PlotArgs::XLim(Some((-50.,50.))))
    //     .set(PlotArgs::YLim(None))
    //     .set(PlotArgs::ZLim(Some((0.,1.))))
    //     .set(PlotArgs::Color(None))
    //     .set(PlotArgs::Backend(PltBackEnd::BMP));

    let plt_type = PlotType::ColorMesh(p_info_params);
    plt_type.plot(&plt_data)?;
    // p_info_params



    // let mut plot = Plot::new(&plt_data, p_info_params);
    // let path = plot.fpath.clone();
    // let backend = BitMapBackend::new(&path, plot.img_size).into_drawing_area();


    // _ = plot_color_mesh2(&mut plot, &backend);

    // plot_2d_line(&PlotData::Dim2(dat_mat2), RGBAColor(255, 0,0, 1.), vec!([true, true],[true, true]), "testx", "testy", &root).unwrap()

Ok(())
}

fn plot_color_mesh2<B:DrawingBackend>(
    plt:            &mut Plot, 
    root: &DrawingArea<B, Shift>
) -> OpmResult<()>{

    if let Some(PlotData::ColorMesh(x,y,dat)) = plt.get_data(){

        _ = root.fill(&WHITE);
        let (root_size_x, _) = root.dim_in_pixel();
        //split root for main plot and colorbar
        let (main_root, cbar_root) = root.split_horizontally(root_size_x as f64*0.85);

        
        let shape = dat.shape();
        let flattened_size = shape.0*shape.1;
        let dat_flat =     MatrixXx1::<f64>::from_iterator(flattened_size, dat.iter().map(|x| *x));
        
        let (x_min, x_max) = if plt.bounds.x.is_none(){
            plt.define_axis_bounds(&DVectorSlice::from(x), false, false)
        }
        else{
            plt.bounds.x.unwrap()
        };
        let (y_min, y_max) = if plt.bounds.y.is_none(){
            plt.define_axis_bounds(&DVectorSlice::from(y), false, false)
        }
        else{
            plt.bounds.y.unwrap()
        };
        let (z_min, z_max) = if plt.bounds.z.is_none(){
            plt.define_axis_bounds(&DVectorSlice::from(&dat_flat), false, false)
        }
        else{
            plt.bounds.z.unwrap()
        };


        //colorrbar. first because otherwise the xlabel of the main plot is cropped
        let mut chart = create_2d_plot_chart(
            &cbar_root,
            (0., 1.),
            (z_min, z_max),
            &"".into(),
            &plt.cbar.label.label,
            &plt.label[0].label_pos,
            &plt.cbar.label.label_pos,
            true, 
            false
            )?;
        
    
        let c_dat = linspace(z_min, z_max, 100).transpose();
        let d_mat = DMatrix::<f64>::from_columns(&[c_dat.clone(), c_dat]);
        let xxx = DVector::<f64>::from_vec(vec![0.,1.]);
        draw_2d_colormesh(
            &mut chart, 
            &xxx, 
            &linspace(z_min, z_max, 100).transpose(), 
            &d_mat,
            &plt.cbar.cmap,
            (z_min, z_max)
        );

        //main plot
        let mut chart = create_2d_plot_chart(
            &main_root,
            (x_min, x_max),
            (y_min, y_max),
            &plt.label[0].label,
            &plt.label[1].label,     
            &plt.label[0].label_pos,
            &plt.label[1].label_pos,       
            true, 
            true          
            )?;

        draw_2d_colormesh(
            &mut chart, 
            &x, 
            &y, 
            &dat,
            &plt.cbar.cmap,
            (z_min, z_max));
                        
    }

    root.present().unwrap();
    
    Ok(())
    }



fn draw_2d_colormesh<'a, T: DrawingBackend>(
    chart:          &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    x:              &MatrixXx1<f64>,
    y:              &MatrixXx1<f64>,
    z:              &DMatrix<f64>,
    cmap:           &Gradient,
    cbounds:        (f64,f64)
){
    let mut x_dist = x[1] - x[0] ;
    if x_dist <= 2.*f64::EPSILON{
        x_dist = 1.
    }
    let y_dist = y[1] - y[0] ;


    let (z_shape_rows, z_shape_cols) = z.shape();
    
    if z_shape_rows!= y.len() || z_shape_cols != x.len(){
        println!("Shapes of x,y and z do not match!");
        return;
    }

    //there will probably a more direct way to achieve the series without thisconversion to a vec<f64> when we can use nalgebra >=v0.32.
    //currently, clone is not implemented for matrix_iter in v0.30 which we use due to ncollide2d. Therefore, we go this way
    let a:Vec<f64> = x.data.to_owned().into();
    let b:Vec<f64> = y.data.to_owned().into();
    let z_min = cbounds.0;
    
    let z_max: f64 = cbounds.1 - z_min;//z.max();
    let series = izip!(iproduct!(a,b),z).map(|((x,y),z)|{
        Rectangle::new(
            [(x, y), (x + x_dist, y -y_dist)],
            {
                let cor = cmap.eval_continuous((z-z_min)/z_max);
                let color = RGBAColor(
                    cor.r,
                    cor.g,
                    cor.b,
                    1.
                );
                Into::<ShapeStyle>::into(color).filled()
            },

        )
    });

    chart.draw_series(series).unwrap();

}

fn create_2d_plot_chart<'a, T: DrawingBackend>(
    root: &'a DrawingArea<T, Shift>,
    x_bounds: (f64,f64),
    y_bounds: (f64,f64),
    xlabel: &String,
    ylabel: &String,
    xlabelpos: &LabelPos,
    ylabelpos: &LabelPos,
    y_ax: bool,
    x_ax: bool
    ) -> OpmResult<ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>> {

        root.fill(&WHITE).unwrap();

        let mut chart_builder = ChartBuilder::on(root);
        chart_builder.margin(15)
        .margin_top(40);

        let (x_px, y_px) = root.dim_in_pixel();   

        if y_ax{
            chart_builder.set_label_area_size(ylabelpos.into(),90);
        }
        chart_builder.set_label_area_size(xlabelpos.into(), 65);
        
        let mut chart = chart_builder
            .build_cartesian_2d(
                x_bounds.0..x_bounds.1, 
                y_bounds.0..y_bounds.1)
            .unwrap();

        let mut mesh = chart
            .configure_mesh();

        mesh.disable_mesh();

        if y_ax{
            mesh.y_desc(ylabel);
        }
        else{
            mesh.disable_y_axis();
        }

        if x_ax{
            mesh.x_desc(xlabel);
        }
        else{
            mesh.disable_x_axis();
        }
        
        mesh.label_style(TextStyle::from(("sans-serif", 30).into_font()))
            .draw()
            .unwrap();   

        Ok(chart)
    }